#import <Cocoa/Cocoa.h>
#import "util.h"
#import "color.h"
#import "front.h"
#import "layout_view.h"

static NSMutableDictionary* font_cache = nil;

static NSFont*
font_for(uint8_t const* name, double size, uint8_t bold, uint8_t italic)
{
    if (!font_cache) {
        font_cache = [NSMutableDictionary dictionary];
        // not fully sure why this is needed...
        // some weird arc stuff i suppose
        [font_cache retain];
    }

    NSString *font_name = name ? [NSString stringWithUTF8String:(const char *)name] : @"SystemFont";
    NSString *cache_key = [NSString stringWithFormat:@"%@;:;-%.2f-%d-%d", font_name, size, bold, italic];

    NSFont *cached_font = [font_cache objectForKey:cache_key];
    if (cached_font) {
        return cached_font;
    }

    NSFontTraitMask traits = 0;
    if (bold) {
        traits |= NSBoldFontMask;
    }
    if (italic) {
        traits |= NSItalicFontMask;
    }

    NSFont *font = nil;
    if (!name) {
        font = [NSFont systemFontOfSize:size];
    }
    else {
        NSURL* fontURL = [NSURL fileURLWithPath:font_name];
        NSArray *descriptors = (NSArray *)CTFontManagerCreateFontDescriptorsFromURL((__bridge CFURLRef)fontURL);
        for (NSFontDescriptor *desc in descriptors) {
            font = [NSFont fontWithDescriptor:desc size:size];
            break;
        }

        if (!font) {
            fprintf(stderr, "quarve: unable to load font %s; defaulting to system\n", name);
            font = [NSFont systemFontOfSize:size];
        }

        [descriptors release];
    }

    NSFontManager *fontManager = [NSFontManager sharedFontManager];
    font = [fontManager convertFont:font toHaveTrait:traits];

    [font_cache setObject:font forKey:cache_key];
    return font;
}

// TODO spent an eternity trying to figure out
// why the font doesn't line up sometimes
// i still don't really know
void *
back_text_init()
{
    NSTextField *label = [[NSTextField alloc] initWithFrame:NSMakeRect(0, 0, 2, 2)];
    [label setEditable:NO];
    [label setSelectable:NO];
    label.bezeled = NO;
    label.drawsBackground = NO;
    return label;
}

void
back_text_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
) {
    NSTextField *tf = (NSTextField *)view;

    NSString *string = [NSString stringWithUTF8String:(const char *)str];
    NSMutableAttributedString *astr = [[NSMutableAttributedString alloc] initWithString:string];
    NSRange fullRange = NSMakeRange(0, string.length);

    NSFont *font = font_for(font_path, font_size, bold, italic);
    [astr addAttribute:NSFontAttributeName value:font range:fullRange];

    if (underline) {
        [astr addAttribute:NSUnderlineStyleAttributeName
                     value:@(NSUnderlineStyleSingle)
                     range:fullRange];
    }

    if (strikethrough) {
        [astr addAttribute:NSStrikethroughStyleAttributeName
                     value:@(NSUnderlineStyleSingle)
                     range:fullRange];
    }

    NSColor *backgroundColor = [NSColor colorWithRed:back.r/255.0
                                               green:back.g/255.0
                                                blue:back.b/255.0
                                               alpha:back.a/255.0];
    [astr addAttribute:NSBackgroundColorAttributeName value:backgroundColor range:fullRange];

    NSColor *foregroundColor = [NSColor colorWithRed:front.r/255.0
                                               green:front.g/255.0
                                                blue:front.b/255.0
                                               alpha:front.a/255.0];
    [astr addAttribute:NSForegroundColorAttributeName value:foregroundColor range:fullRange];

    if (![tf.attributedStringValue isEqualToAttributedString: astr]) {
        tf.attributedStringValue = astr;
        tf.font = font;
        tf.textColor = foregroundColor;

        // FIXME, find a better solution
        // although I'm not sure if you can actually even edit the attributes??
        // but this preserves the attributes upon selection
        tf.allowsEditingTextAttributes = underline || strikethrough || back.a != 0;
    }
    [astr release];

    tf.maximumNumberOfLines = max_lines;
}

size
back_text_size(void* view, size suggested)
{
    NSTextField* tf = view;
    NSCell *cell = [tf cell];
    NSSize s = [cell cellSizeForBounds:NSMakeRect(0, 0, suggested.w, suggested.h)];
    return (size) { s.width, s.height };
}

// MARK: textfield
@interface FieldEditor : NSTextView
@end

@interface TextField : NSTextField<NSTextFieldDelegate>
@property fat_pointer focused;
@property int32_t focused_token;
@property fat_pointer text;
@property fat_pointer callback;
@property BOOL scheduled_focused;
@end

@implementation TextField

- (void) textDidChange:(NSNotification *)notification {
    front_set_opt_string_binding(self.text, (uint8_t *const) [self.stringValue UTF8String]);
}

- (void)viewDidMoveToWindow {
    if (self.scheduled_focused) {
        [self becomeFirstResponder];
    }
}

// make first responder
- (void) updateFocus {
    if (self.currentEditor != nil) {
        front_set_token_binding(self.focused, 1, self.focused_token);
    }
    else {
        front_set_token_binding(self.focused, 0, self.focused_token);
    }
}

- (BOOL)becomeFirstResponder
{
    BOOL status = [super becomeFirstResponder];
    if (status) {
        front_set_token_binding(self.focused, 1, self.focused_token);
    }
    return status;
}

- (void)controlTextDidBeginEditing:(NSNotification *)obj {
    front_set_token_binding(self.focused, 1, self.focused_token);
}

- (void) mouseDown:(NSEvent*)event {
    [super mouseDown:event];

    [self updateFocus];
}

// for some reason contextual menus
// dont invoke becomeFirstResponder at all??
- (void) rightMouseDown:(NSEvent*)event {
    front_set_token_binding(self.focused, 1, self.focused_token);
    [super rightMouseDown:event];

    [self updateFocus];
}

- (NSMenu *)menuForEvent:(NSEvent *)event {
    NSMenu* ret = [super menuForEvent:event];

    [self updateFocus];
    return ret;
}

// resign first responder
- (void)controlTextDidEndEditing:(NSNotification *)aNotification {
    front_set_token_binding(self.focused, 0, self.focused_token);
}

// action
- (void)action:(id)sender {
    front_execute_fn_mut(self.callback);
 }

 - (void)keyDown:(NSEvent *)event {
     if (event.keyCode == 53) { // 53 is the key code for Escape
         [self.window makeFirstResponder:nil];
     } else {
         [super keyDown:event];
     }
}
// key
- (BOOL)control:(NSControl*)control textView:(NSTextView*)textView doCommandBySelector:(SEL)commandSelector
{
    BOOL result = NO;

    if (commandSelector == @selector(insertTab:))
    {
        [self.window makeFirstResponder:nil];
        front_set_token_binding(self.focused, 1, self.focused_token  + 1);

        result = YES;
    }
    else if (commandSelector == @selector(insertBacktab:))
    {
        [self.window makeFirstResponder:nil];
        front_set_token_binding(self.focused, 1, self.focused_token - 1);

        result = YES;
    }

    return result;
}

- (void)dealloc {
    [super dealloc];

    front_free_token_binding(self.focused);
    front_free_opt_string_binding(self.text);
    front_free_fn_mut(self.callback);
}
@end

void*
back_text_field_init(
    fat_pointer text_binding,
    fat_pointer focused_binding,
    fat_pointer callback,
    int32_t token,
    uint8_t unstyled,
    uint8_t secure
)
{
    (void) secure;

    TextField* tf = [[TextField alloc] init];
    tf.focused = focused_binding;
    tf.text = text_binding;
    tf.callback = callback;
    tf.focused_token = token;
    tf.scheduled_focused = NO;
    tf.drawsBackground = NO;
    tf.bordered = NO;
    tf.bezeled = NO;

    if (unstyled) {
        tf.focusRingType = NSFocusRingTypeNone;
    }

    [tf setAction:@selector(action:)];
    [tf setTarget:tf];

    tf.delegate = tf;
    return tf;
}

void
back_text_field_focus(void *view)
{
    TextField* tf = view;
    tf.scheduled_focused = YES;
    if (tf.currentEditor == nil) {
        [tf becomeFirstResponder];
    }
}

void
back_text_field_unfocus(void *view)
{
    TextField* tf = view;
    tf.scheduled_focused = NO;
    if (tf.currentEditor != nil) {
        [tf.window makeFirstResponder:nil];
    }
}

void
back_text_field_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
)
{
    back_text_update(
        view,
        str,
        max_lines,
        bold, italic, underline, strikethrough,
        back, front,
        font_path,
        font_size
    );
}

size
back_text_field_size(void* view, size suggested)
{
    NSTextField* tf = view;
    NSCell *cell = [tf cell];
    NSSize s = [cell cellSizeForBounds:NSMakeRect(0, 0, suggested.w, suggested.h)];
    return (size) { s.width, s.height };
}

void
back_text_field_select_all(void *view)
{
    TextField* tf = view;
    [tf.currentEditor selectAll:view];
}

void
back_text_field_cut(void *view)
{
    TextField* tf = view;
    [tf.currentEditor cut:view];
}

void
back_text_field_copy(void *view)
{
    TextField* tf = view;
    [tf.currentEditor copy:view];
}

void
back_text_field_paste(void *view)
{
    TextField* tf = view;
    [tf.currentEditor paste:view];
}

// MARK: textview
@interface TextView : NSTextView<NSTextViewDelegate>
@property fat_pointer _state;
@property fat_pointer selected;
@property fat_pointer key_handler;

@property int32_t page_id;
@property BOOL executing_back;
@property BOOL dragging;
@end

@implementation TextView
- (BOOL)textView:(NSTextView *)textView doCommandBySelector:(SEL)commandSelector {
    if (commandSelector == @selector(cancelOperation:)) {
        if (!front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_ESCAPE)) {
            [textView.window makeFirstResponder:nil];
        }
        return YES;
    }
    else if (commandSelector == @selector(moveUp:)) {
        if (front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_UP)) {
            return YES;
        }
    }
    else if (commandSelector == @selector(moveDown:)) {
        if (front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_DOWN)) {
            return YES;
        }
    }
    else if (commandSelector == @selector(moveLeft:)) {
        if (front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_LEFT)) {
            return YES;
        }
    }
    else if (commandSelector == @selector(moveRight:)) {
        if (front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_RIGHT)) {
            return YES;
        }
    }

    return NO;
}

- (BOOL)shouldChangeTextInRange:(NSRange)affectedCharRange
              replacementString:(NSString *)replacementString {
    if (!self.executing_back) {
        if (replacementString) {
            uint8_t const* const str = (uint8_t const*) [replacementString UTF8String];
            front_replace_textview_range(self._state, affectedCharRange.location, affectedCharRange.length, str);
        }
        else {
            uint8_t const* const str = (uint8_t const*) "";
            front_replace_textview_range(self._state, affectedCharRange.location, affectedCharRange.length, str);
        }
    }

    return YES;
}

- (void)textViewDidChangeSelection:(NSNotification *)notification
{
    if (!self.executing_back) {
        NSRange range = [self selectedRange];
        front_set_textview_selection(self._state, range.location, range.length);
    }
}

- (BOOL)becomeFirstResponder {
    front_set_token_binding(self.selected, 1, (int32_t) self.page_id);
    return [super becomeFirstResponder];
}

- (BOOL)resignFirstResponder {
    front_set_token_binding(self.selected, 0, 0);
    return [super resignFirstResponder];
}

- (void)insertTab:(id)sender {
    if (!front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_TAB)) {
        [super insertTab: sender];
    }
}

- (void)insertBacktab:(id) sender {
    if (!front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_UNTAB)) {
        [super insertBacktab:sender];
    }
}

- (void)insertNewline:(id) sender {
    if (!front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_NEWLINE)) {
        [super insertNewline:sender];
    }
}

- (void)insertNewlineIgnoringFieldEditor:(id) sender {
    if (!front_execute_key_callback(self.key_handler, TEXTVIEW_CALLBACK_KEYCODE_ALT_NEWLINE)) {
        [super insertNewlineIgnoringFieldEditor:sender];
    }
}

- (void)dealloc {
    [super dealloc];

    front_free_token_binding(self.selected);
    front_free_textview_state(self._state);
    front_free_key_callback(self.key_handler);
}
@end

void *
back_text_view_init()
{
    TextView* tv = [[TextView alloc] initWithFrame:NSMakeRect(0, 0,
               1000, 500)];

    [tv setHorizontallyResizable:NO];
    [tv setVerticallyResizable:YES];
    [[tv textContainer]
                setContainerSize:NSMakeSize(FLT_MAX, FLT_MAX)];
    [[tv textContainer] setWidthTracksTextView:YES];
    [[tv textContainer] setHeightTracksTextView:NO];
    [tv setAutoresizingMask:NSViewWidthSizable];

    tv.page_id = 0;

    tv.textContainerInset = NSMakeSize(0, 0);
    [tv textContainer].lineFragmentPadding = 0;

    tv.drawsBackground = NO;
    tv.richText = NO;
    tv.editable = YES;
    tv.allowsUndo = NO;
    tv.automaticSpellingCorrectionEnabled = NO;
    tv.automaticTextReplacementEnabled = NO;
    tv.automaticTextCompletionEnabled = NO;
    tv.automaticQuoteSubstitutionEnabled = NO;
    tv.automaticDashSubstitutionEnabled = NO;
    tv.automaticLinkDetectionEnabled = NO;
    tv.automaticDataDetectionEnabled = NO;
    tv.continuousSpellCheckingEnabled = NO;
    tv.enabledTextCheckingTypes = 0;
    tv.grammarCheckingEnabled = NO;
    tv.smartInsertDeleteEnabled = NO;
    [tv turnOffKerning:nil];
    [tv turnOffLigatures:nil];
    tv.delegate = tv;

    return tv;
}

// may discard attributes
void
back_text_view_full_replace(
    void *tv,
    const uint8_t* with,
    fat_pointer _state,
    fat_pointer selected,
    fat_pointer key_callback
)
{
    TextView* textView = tv;
    textView.executing_back = YES;

    textView.string = [NSString stringWithUTF8String:(const char*)with];

    textView.selected = selected;
    textView._state = _state;
    textView.key_handler = key_callback;

    textView.executing_back = NO;
}

void
back_text_view_replace(void *tv, size_t start, size_t len, const uint8_t* with)
{
    TextView* textView = tv;
    textView.executing_back = YES;
    [textView replaceCharactersInRange: NSMakeRange(start, len)
                      withString: [NSString stringWithUTF8String:(const char*)with]
    ];
    textView.executing_back = NO;
}

void
back_text_view_set_font(
    void *tv, uint8_t const* font_path, double font_size
)
{
    TextView* textView = tv;
    NSFont *nsFont = font_for(font_path, font_size, 0, 0);

    textView.executing_back = YES;

    [textView setFont:nsFont];

    textView.executing_back = NO;
}

void
back_text_view_set_editing_state(void *tv, uint8_t editing, uint8_t first_editing_block)
{
    TextView* textView = tv;
    if (editing) {
        if (first_editing_block) {
            textView.needsDisplay = YES;
        }
        [textView.textStorage beginEditing];
    }
    else {
        [textView.textStorage endEditing];
    }
}

void
back_text_view_set_line_attributes(
    void *tv,
    size_t line_no, size_t start, size_t end,
    int justification_sign,
    double leading_indentation, double trailing_indentation
)
{
    (void) line_no;

    TextView* textView = tv;
    textView.needsDisplay = YES;
    textView.executing_back = YES;

    NSRange range = NSMakeRange(start, end - start);

    [textView.textStorage removeAttribute:NSBackgroundColorAttributeName range:range];
    [textView.textStorage removeAttribute:NSUnderlineStyleAttributeName range:range];
    [textView.textStorage removeAttribute:NSStrikethroughStyleAttributeName range:range];
    NSFontTraitMask fontTraits = NSUnboldFontMask | NSUnitalicFontMask;
    [textView.textStorage applyFontTraits:fontTraits range:range];

    NSMutableParagraphStyle *paragraphStyle = [[NSMutableParagraphStyle alloc] init];

    if (justification_sign < 0) {
        paragraphStyle.alignment = NSTextAlignmentLeft;
    } else if (justification_sign == 0) {
        paragraphStyle.alignment = NSTextAlignmentCenter;
    } else {
        paragraphStyle.alignment = NSTextAlignmentRight;
    }

    paragraphStyle.firstLineHeadIndent = leading_indentation;
    paragraphStyle.headIndent = leading_indentation;
    paragraphStyle.tailIndent = -trailing_indentation;

    [textView.textStorage addAttribute:NSParagraphStyleAttributeName value:paragraphStyle range:range];

    [paragraphStyle release];

    textView.executing_back = NO;
}

void
back_text_view_set_char_attributes(
    void *tv, size_t start, size_t end,
    uint8_t bold, uint8_t italic, uint8_t underline, uint8_t strikethrough,
    color back, color front
)
{
    TextView* textView = tv;
    textView.executing_back = YES;
    textView.needsDisplay = YES;

    NSRange range = NSMakeRange(start, end - start);

    // Create a single dictionary for attributes to be added
    NSMutableDictionary *attributes = [NSMutableDictionary dictionary];

    NSFontTraitMask fontTraits = 0;
    if (bold) {
        fontTraits |= NSBoldFontMask;
    }
    if (italic) {
        fontTraits |= NSItalicFontMask;
    }

    if (fontTraits != 0) {
        [textView.textStorage applyFontTraits:fontTraits range:range];
    }

    if (underline) {
        attributes[NSUnderlineStyleAttributeName] = @(NSUnderlineStyleSingle);
    }

    if (strikethrough) {
        attributes[NSStrikethroughStyleAttributeName] = @(NSUnderlineStyleSingle);
    }

    NSColor *backgroundColor = [NSColor colorWithRed:back.r/255.0
                                               green:back.g/255.0
                                                blue:back.b/255.0
                                               alpha:back.a/255.0];
    attributes[NSBackgroundColorAttributeName] = backgroundColor;

    NSColor *foregroundColor = [NSColor colorWithRed:front.r/255.0
                                               green:front.g/255.0
                                                blue:front.b/255.0
                                               alpha:front.a/255.0];
    attributes[NSForegroundColorAttributeName] = foregroundColor;

    [textView.textStorage addAttributes:attributes range:range];

    textView.executing_back = NO;
}

void
back_text_view_set_selection(void *tv, size_t start, size_t len)
{
    TextView* textView = tv;
    textView.executing_back = YES;

    NSRange range = NSMakeRange(start, len);
    if (!NSEqualRanges(range, [textView selectedRange])) {
        [textView setSelectedRange: range];
    }

    textView.executing_back = NO;
}

void
back_text_view_get_selection(void *tv, size_t *restrict start, size_t* restrict end)
{
    TextView* textView = tv;
    NSRange range = [textView selectedRange];

    *start = range.location;
    *end = range.location + range.length;
}

double
back_text_view_get_line_height(void *tv, size_t line, size_t start, size_t end, double width)
{
    (void) line;
    (void) width;

    TextView* textView = tv;
    textView.executing_back = YES;

    NSRange charRange = NSMakeRange(start, end - start);
    NSRange glyphRange = [textView.layoutManager glyphRangeForCharacterRange:charRange
                                                 actualCharacterRange:nil];
    CGFloat ret = [textView.layoutManager boundingRectForGlyphRange:glyphRange
                                          inTextContainer:textView.textContainer].size.height;

    textView.executing_back = NO;
    return ret;
}

void
back_text_view_get_cursor_pos(void *tv, double *x, double *y)
{
    TextView* textView = tv;

    NSRange charRange = [textView selectedRange];
    NSRange glyphRange = [textView.layoutManager glyphRangeForCharacterRange:charRange
                                                 actualCharacterRange:nil];
    NSRect ret = [textView.layoutManager boundingRectForGlyphRange:glyphRange
                                          inTextContainer:textView.textContainer];
    *x = ret.origin.x;
    *y = ret.origin.y;
}


void
back_text_view_set_page_id(void *tv, int32_t page_id)
{
    TextView* textView = tv;
    textView.page_id = page_id;
}

void
back_text_view_focus(void *tv)
{
    TextView* textView = tv;
    if (textView.window.firstResponder != textView) {
        [textView.window makeFirstResponder:textView];
    }
}

void
back_text_view_unfocus(void *tv)
{
    TextView* textView = tv;
    if (textView.window.firstResponder == textView) {
        [textView.window makeFirstResponder:nil];
    }
}

void
back_text_view_copy(void *tv)
{
    TextView *textView = tv;
    [textView copy:textView];
}

void
back_text_view_cut(void *tv)
{
    TextView *textView = tv;
    [textView cut:textView];
}

void
back_text_view_paste(void *tv)
{
    TextView *textView = tv;
    [textView paste:textView];
}

void
back_text_view_select_all(void *tv)
{
    TextView *textView = tv;
    [textView selectAll:textView];
}
