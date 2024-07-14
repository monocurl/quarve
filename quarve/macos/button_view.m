#import <Cocoa/Cocoa.h>
#import "cursor_view.h"
#import "util.h"

void *
back_view_button_init() {
    CursorView* view = [[CursorView alloc] initWithCursor:[NSCursor pointingHandCursor]];
    [view setWantsLayer: YES];
    return view;
}

void
back_view_button_update(void *_view, uint8_t clicked) {
    CursorView* view = _view;
    if (clicked) {
        view.layer.opacity = 0.75;
    }
    else {
        view.layer.opacity = 1.0;
    }
}
