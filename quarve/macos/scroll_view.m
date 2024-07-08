#import <Cocoa/Cocoa.h>
#import "quarve_macos.h"

extern int performing_subview_insertion;

@interface ScrollView : NSScrollView
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
    printf("Scrolled %f\n", scrollPosition.y);
}

@end

void *
back_view_scroll_init(uint8_t allow_vertical, uint8_t allow_horizontal)
{
    ScrollView* scroll = [[ScrollView alloc] init];
    scroll.drawsBackground = NO;
    scroll.allowsVertical = allow_vertical;
    scroll.allowsHorizontal = allow_horizontal;

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