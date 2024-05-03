#import <Cocoa/Cocoa.h>
#import "quarve_macos.h"

/* front end */
extern void front_will_spawn(void);

// box: &'static dyn WindowBase
extern bool front_window_should_close(fat_pointer p);

// box: &dyn FnOnce(&Slock<MainThreadMarker>) + Send + 'static
extern void front_execute_box(fat_pointer box);

@interface AppDelegate : NSObject<NSApplicationDelegate>
- (void)applicationWillFinishLaunching:(NSNotification *)aNotification;
@end

@implementation AppDelegate
- (void)applicationWillFinishLaunching:(NSNotification *)aNotification {
    // TODO maybe move to applicationDidFinishLaunching
    front_will_spawn();
}
@end

@interface Window : NSWindow {
    /* callbacks */
    fat_pointer handle;

    NSBox* groupBox1;
    NSBox* groupBox2;
}
- (instancetype)init: (fat_pointer) handle;
- (BOOL)windowShouldClose:(id)sender;
@end

@implementation Window
- (instancetype)init: (fat_pointer) raw_handle {
    handle = raw_handle;

    groupBox1 = [[NSBox alloc] initWithFrame:NSMakeRect(10, 10, 305, 460)];
    [groupBox1 setTitle:@"GroupBox 1"];

    groupBox2 = [[NSBox alloc] initWithFrame:NSMakeRect(325, 10, 305, 460)];
    [groupBox2 setTitle:@""];

    [super initWithContentRect:NSMakeRect(100, 100, 640, 480) styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
    [self setTitle:@"GroupBox example"];
    [[self contentView] addSubview:groupBox1];
    [[self contentView] addSubview:groupBox2];
    [self setIsVisible:YES];

    return self;
}

- (BOOL)windowShouldClose:(id)sender {
    return (BOOL) front_window_should_close(handle);
}
@end

void
back_main_loop() {
    @autoreleasepool {
        NSApplication *application = [NSApplication sharedApplication];
        [application setActivationPolicy:NSApplicationActivationPolicyRegular];

        AppDelegate * dg = [[AppDelegate alloc] init];
        [application setDelegate: dg];

        [application run];
    }
}

void *
back_register_window(fat_pointer handle) {
    return [[Window alloc] init:handle];
}

void
back_exit_window(void *window_p) {
    Window* window = window_p;
    [window close];
}

void
back_run_main(fat_pointer box) {
    dispatch_async(dispatch_get_main_queue(), ^(void){
        front_execute_box(box);
    });
}

void
back_terminate() {
    [NSApp terminate:nil];
}
