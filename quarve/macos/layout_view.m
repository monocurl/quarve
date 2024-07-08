#import <Cocoa/Cocoa.h>
#import "util.h"

@interface LayoutView: NSView
@end

@implementation LayoutView
- (BOOL)isFlipped {
    return YES;
}
@end

void *
back_view_layout_init() {
    return [[LayoutView alloc] init];
}
