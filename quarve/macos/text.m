#import <Cocoa/Cocoa.h>
#import "util.h"
#import "color.h"
#import "front.h"

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

    NSFont *cached_font = [font_cache objectForKey:cache_key];;
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
    }

    NSFontManager *fontManager = [NSFontManager sharedFontManager];
    font = [fontManager convertFont:font toHaveTrait:traits];

    [font_cache setObject:font forKey:cache_key];
    return font;
}

void*
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
    NSTextField* tf = view;

    NSString* string = [NSString stringWithUTF8String:(const char *)str];

    NSMutableAttributedString* astr = [[NSMutableAttributedString alloc] initWithString:string];
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
void
back_text_view_init()
{

}
