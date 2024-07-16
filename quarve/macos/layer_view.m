#import <Cocoa/Cocoa.h>
#import "color.h"
#import "util.h"

@interface LayerView: NSView
@end

@implementation LayerView
- (BOOL) isFlipped {
    return YES;
}
@end

void *
back_view_layer_init() {
    NSView* ret = [[NSView alloc] init];
    [ret setWantsLayer: YES];
    return ret;
}

void
back_view_layer_update(void *_view, color background_color, color border_color, double corner_radius, double border_width, float opacity)
{
    NSView* view = _view;
    view.layer.borderWidth = border_width;
    view.layer.cornerRadius = corner_radius;

    CGColorRef bg = color_to_cg_color(background_color);
    view.layer.backgroundColor = bg;
    CGColorRelease(bg);

    CGColorRef border = color_to_cg_color(border_color);
    view.layer.borderColor = border;
    CGColorRelease(border);
    view.layer.opacity = opacity;
}
