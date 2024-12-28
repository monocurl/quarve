#import <Cocoa/Cocoa.h>
#import "util.h"
#import "front.h"

extern int performing_subview_insertion;

@interface ScrollView : NSScrollView
@property fat_pointer binding_x;
@property fat_pointer binding_y;
@property double last_x;
@property double last_y;
@property BOOL ignore_scroll;
@property BOOL allowsVertical;
@property BOOL allowsHorizontal;
@end

@implementation ScrollView

- (void)addSubview:(NSView *)view
         positioned:(NSWindowOrderingMode)place
         relativeTo:(NSView *)otherView {
         if (performing_subview_insertion) {
            [self setDocumentView:view];
         }
         else {
            [super addSubview:view positioned:place relativeTo:otherView];
        }
}

- (void)scrollWheel:(NSEvent *)event {
    CGEventRef cgEvent = [event CGEvent];
    if (cgEvent) {
        CGEventRef mutableEvent = CGEventCreateCopy(cgEvent);
        if (!self.allowsVertical) {
            CGEventSetDoubleValueField(mutableEvent, kCGScrollWheelEventDeltaAxis1, 0.0);
            CGEventSetDoubleValueField(mutableEvent, kCGScrollWheelEventPointDeltaAxis1, 0.0);
        }
        if (!self.allowsHorizontal) {
            CGEventSetDoubleValueField(mutableEvent, kCGScrollWheelEventDeltaAxis2, 0.0);
            CGEventSetDoubleValueField(mutableEvent, kCGScrollWheelEventPointDeltaAxis2, 0.0);
        }
        NSEvent *adjustedEvent = [NSEvent eventWithCGEvent:mutableEvent];
        CFRelease(mutableEvent);
        [super scrollWheel:adjustedEvent];
    } else {
        [super scrollWheel:event];
    }
}

- (void)didScroll:(NSNotification *)notification {
    NSRect bounds = [self.contentView bounds];
    NSPoint scrollPosition = bounds.origin;

    if (!self.ignore_scroll && (fabs(scrollPosition.x - self.last_x) > EPSILON || fabs(scrollPosition.y - self.last_y) > EPSILON)) {
        self.last_x = scrollPosition.x;
        self.last_y = scrollPosition.y;
        front_set_screen_unit_binding(self.binding_x, scrollPosition.x);
        front_set_screen_unit_binding(self.binding_y, scrollPosition.y);
    }
}

- (void)dealloc {
    [super dealloc];

    front_free_screen_unit_binding(self.binding_x);
    front_free_screen_unit_binding(self.binding_y);
}

@end

void *
back_view_scroll_init(
    uint8_t allow_vertical,
    uint8_t allow_horizontal,
    fat_pointer vertical_offset,
    fat_pointer horizontal_offset
)
{
    ScrollView* scroll = [[ScrollView alloc] init];
    scroll.drawsBackground = NO;
    scroll.allowsVertical = allow_vertical;
    scroll.allowsHorizontal = allow_horizontal;
    scroll.binding_x = horizontal_offset;
    scroll.binding_y = vertical_offset;
    scroll.ignore_scroll = NO;

    if (allow_vertical) {
        scroll.hasVerticalScroller = YES;
    }

    if (allow_horizontal) {
        scroll.hasHorizontalScroller = YES;
    }

    [scroll.contentView setPostsBoundsChangedNotifications:YES];
    [[NSNotificationCenter defaultCenter]
        addObserver:scroll
        selector:@selector(didScroll:)
        name:NSViewBoundsDidChangeNotification
        object:scroll.contentView];

    return scroll;
}

void
back_view_scroll_set_x(void *backing, double value)
{
    ScrollView* scroll = backing;

    NSRect bounds = [scroll.contentView bounds];
    NSPoint scrollPosition = bounds.origin;
    if (fabs(value - scrollPosition.x) > EPSILON) {
        scroll.last_x = value;

        scroll.ignore_scroll = YES;
        scrollPosition.x = value;
        [[scroll contentView] scrollPoint:scrollPosition];
        [scroll reflectScrolledClipView: [scroll contentView]];
        scroll.ignore_scroll = NO;
    }
}

void
back_view_scroll_set_y(void *backing, double value)
{
    ScrollView* scroll = backing;

    NSRect bounds = [scroll.contentView bounds];
    NSPoint scrollPosition = bounds.origin;
    if (fabs(value - scrollPosition.y) > EPSILON) {
        scroll.last_y = value;

        scroll.ignore_scroll = YES;
        scrollPosition.y = value;
        [[scroll contentView] scrollPoint:scrollPosition];
        [scroll reflectScrolledClipView: [scroll contentView]];
        scroll.ignore_scroll = NO;
    }
}

@interface ContentView: NSView
@end

@implementation ContentView
- (BOOL) isFlipped {
    return YES;
}
@end

void *
back_view_scroll_content_init()
{
    return [[ContentView alloc] init];
}
