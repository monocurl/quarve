#import <Cocoa/Cocoa.h>
#import "cursor_view.h"
#import "util.h"

@interface ButtonView : CursorView
@end

@implementation ButtonView
@end

void *
back_view_button_init() {
    ButtonView* view = [[ButtonView alloc] initWithCursor:[NSCursor pointingHandCursor]];
    [view setWantsLayer: YES];
    return view;
}

void
back_view_button_update(void *_view, uint8_t clicked) {
    ButtonView* view = _view;
    if (clicked) {
        view.layer.opacity = 0.75;
    }
    else {
        view.layer.opacity = 1.0;
    }
}
