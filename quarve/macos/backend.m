#import <Cocoa/Cocoa.h>

@interface AppDelegate : NSObject <NSApplicationDelegate>

@property (strong, nonatomic) NSWindow *window;

@end

@implementation AppDelegate

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    // Create a window with a frame
    NSRect frame = NSMakeRect(0, 0, 400, 200);
    self.window = [[NSWindow alloc] initWithContentRect:frame
                                               styleMask:(NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable)
                                                 backing:NSBackingStoreBuffered
                                                   defer:NO];
    [self.window setTitle:@"Quarve"];

    // Display the window
    [self.window center];
    [self.window makeKeyAndOrderFront:nil];
}

@end

void launch_window() {
    @autoreleasepool {
        // Create the application instance
        NSApplication *application = [NSApplication sharedApplication];

        // Create an instance of AppDelegate
        AppDelegate *appDelegate = [[AppDelegate alloc] init];

        // Set the application's delegate
        [application setDelegate:appDelegate];

        // Run the application event loop
        [application run];
    }
}
