#import <Cocoa/Cocoa.h>
#import "quarve_macos.h"

@interface CursorView : NSView
- (instancetype)initWithCursor:(NSCursor *)cursor;
@property (strong) NSCursor *cursor;
@end

@implementation CursorView

- (instancetype)initWithCursor:(NSCursor *)cursor {
    [super init];
    self.cursor = cursor;

    [self setWantsLayer: YES];
    return self;
}

- (void)resetCursorRects {
    [self discardCursorRects];
    [self addCursorRect:[self visibleRect] cursor:self.cursor];
}
@end

static NSCursor*
from_quarve_cursor(int cursor_type)
{
    NSCursor* cursor;
    if (cursor_type == CURSOR_ARROW) {
        cursor = [NSCursor arrowCursor];
    }
    else if (cursor_type == CURSOR_IBEAM) {
        cursor = [NSCursor IBeamCursor];
    }
    else if (cursor_type == CURSOR_POINTER) {
        cursor = [NSCursor pointingHandCursor];
    }
    else {
        NSLog(@"Invalid cursor requested!");
        cursor = [NSCursor arrowCursor];
    }

    return cursor;
}

void *
back_view_cursor_init(int cursor_type) {
    NSCursor* c = from_quarve_cursor(cursor_type);
    return [[CursorView alloc] initWithCursor:c];
}

void
back_view_cursor_update(void *_view, int cursor_type) {
    NSCursor* c = from_quarve_cursor(cursor_type);

    CursorView* view = _view;
    view.cursor = c;
    [view.window invalidateCursorRectsForView: view];
}
