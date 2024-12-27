#import <Cocoa/Cocoa.h>
#import "cursor_view.h"
#import "../inc/util.h"

@implementation CursorView

- (instancetype)initWithCursor:(NSCursor *)cursor {
    [super init];
    self.cursor = cursor;

    // for button view
    [self setWantsLayer: YES];
    return self;
}

- (void)resetCursorRects {
    [self discardCursorRects];

    NSScrollView *scrollView = [self enclosingScrollView];

    if (scrollView) {
        NSRect bound_indoc = [self convertRect:self.bounds toView:scrollView.documentView];
        NSRect doc_rect = scrollView.documentVisibleRect;
        NSRect clipped_indoc = NSIntersectionRect(bound_indoc, doc_rect);
        NSRect full = [self convertRect:clipped_indoc fromView:scrollView.documentView];

        [self addCursorRect:full cursor:self.cursor];
    } else {
        [self addCursorRect:self.bounds cursor:self.cursor];
    }
}

- (BOOL)isFlipped {
    return YES;
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
