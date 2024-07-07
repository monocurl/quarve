#import <Cocoa/Cocoa.h>
#import "quarve_macos.h"

void *
back_view_scroll_init(uint8_t is_vertical)
{
    NSScrollView* scroll = [[NSScrollView alloc] initWithFrame: NSMakeRect(0,0,2,2)];
    return scroll;
}