#import <Cocoa/Cocoa.h>
#import "util.h"

@interface LayoutView: NSView
@end

@implementation LayoutView
- (BOOL)isFlipped {
    return YES;
}

- (NSView *)hitTest:(NSPoint)point {
    NSView *hitView = [super hitTest:point];
    // exclude this view from hit tests
    return hitView == self ? nil : hitView;
}
@end

void *
back_view_layout_init() {
    return [[LayoutView alloc] init];
}
