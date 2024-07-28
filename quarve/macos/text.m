#import <Cocoa/Cocoa.h>
#import "util.h"
#import "color.h"
#import "front.h"

static NSFont*
font_for(uint8_t const* name, double size, uint8_t bold, uint8_t italic)
{
    static NSMutableDictionary* font_cache = nil;
    if (!font_cache) {
        font_cache = [NSMutableDictionary dictionary];
    }

    NSString *font_name = name ? [NSString stringWithUTF8String:(const char *)name] : @"SystemFont";
    NSString *cache_key = [NSString stringWithFormat:@"%@-%.2f-%d-%d", font_name, size, bold, italic];

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
@interface TextField : NSTextField
@property fat_pointer focused;
@property fat_pointer text;
@end

@implementation TextField

// text did Change

// make first responder

// resign first responder

@end

void*
back_text_field_init(fat_pointer text_binding, fat_pointer focused_binding)
{
    TextField* tf = [[TextField alloc] init];
    tf.focused = focused_binding;
    tf.text = text_binding;
    return tf;
}

void
back_text_field_focus(void *view)
{
    TextField* tf = view;
    [tf becomeFirstResponder];
}

void
back_text_field_unfocus(void *view)
{
    TextField* tf = view;
    [tf resignFirstResponder];
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


// MARK: textview
void
back_text_view_init()
{

}
