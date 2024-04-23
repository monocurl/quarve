#import <Cocoa/Cocoa.h>

void main_loop() {
    @autoreleasepool {
        NSApplication *application = [NSApplication sharedApplication];
	    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        NSRect frame = NSMakeRect(0, 0, 400, 200);
        NSWindow* window = [[NSWindow alloc] initWithContentRect:frame
                                                       styleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable)
                                                         backing:NSBackingStoreBuffered
                                                           defer:NO];
        [window setTitle:@"Quarve"];

        // Display the window
        [window center];
        [window makeKeyAndOrderFront:nil];

        [application run];
    }
}