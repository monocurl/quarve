#import <Cocoa/Cocoa.h>
#import "color.h"
#import "util.h"
#import "front.h"

/* internal state */
int performing_subview_insertion = 0;

@interface AppDelegate : NSObject<NSApplicationDelegate>
- (void)applicationWillFinishLaunching:(NSNotification *)aNotification;
@end

@implementation AppDelegate
- (void)applicationWillFinishLaunching:(NSNotification *)aNotification {
    // FIXME maybe move to applicationDidFinishLaunching
    front_will_spawn();
}
@end

@interface ContentView : NSView
@end

@interface Window : NSWindow<NSWindowDelegate> {
    /* callbacks */
    @public fat_pointer handle;
}
@end

@implementation ContentView
- (void)layout {
    Window* window = (Window*) self.window;
    NSRect content = [window contentRectForFrameRect:window.frame];
    front_window_layout(window->handle, (double) NSWidth(content), (double) NSHeight(content));

    [super layout];
}

- (BOOL)isFlipped {
    return YES;
}

@end

@implementation Window
- (instancetype)init {
    [super initWithContentRect:NSMakeRect(0, 0, 2, 2) styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
    [self setIsVisible:YES];

    self.releasedWhenClosed = NO;
    self.delegate = self;

    return self;
}
- (void) setHandle:(fat_pointer) _handle {
    self->handle = _handle;

    ContentView *contentView = [[ContentView alloc] initWithFrame:NSMakeRect(0, 0, 100, 100)];
    [self setContentView:contentView];
    [self center];
}

- (BOOL)windowShouldClose:(id)sender {
    return (BOOL) front_window_should_close(handle);
}

- (void)dispatchEvent:(NSEvent*)event {
    buffer_event be = { .native_event = event };

    be.cursor_x = event.locationInWindow.x;
    be.cursor_y = event.locationInWindow.y;

    if (event.modifierFlags & NSEventModifierFlagCommand) {
        be.modifiers |= EVENT_MODIFIER_COMMAND;
    }
    if (event.modifierFlags & NSEventModifierFlagControl) {
        be.modifiers |= EVENT_MODIFIER_CONTROL;
    }
    if (event.modifierFlags & NSEventModifierFlagShift) {
        be.modifiers |= EVENT_MODIFIER_SHIFT;
    }
    if (event.modifierFlags & NSEventModifierFlagFunction) {
        be.modifiers |= EVENT_MODIFIER_FN;
    }
    if (event.modifierFlags & NSEventModifierFlagOption) {
        be.modifiers |= EVENT_MODIFIER_ALT_OPTION;
    }

    if (event.type == NSEventTypeKeyUp) {
        be.is_up = 1;
        be.key_characters = (unsigned char const *) event.characters.UTF8String;
    }
    else if (event.type == NSEventTypeKeyDown) {
        if (!event.ARepeat) {
            be.is_down = 1;
        }
        be.key_characters = (unsigned char const *) event.characters.UTF8String;
    }
    else if (event.type == NSEventTypeScrollWheel) {
        be.is_mouse = 1;
        be.is_scroll = 1;
        be.delta_x = event.scrollingDeltaX;
        be.delta_y = event.scrollingDeltaY;
    }
    else if (event.type == NSEventTypeLeftMouseDown) {
        be.is_mouse = 1;
        be.is_left_button = 1;
        be.is_down = 1;
    }
    else if (event.type == NSEventTypeLeftMouseUp) {
        be.is_mouse = 1;
        be.is_left_button = 1;
        be.is_up = 1;
    }
    else if (event.type == NSEventTypeLeftMouseDragged) {
        be.is_mouse = 1;
        be.is_left_button = 1;
        be.delta_x = event.deltaX;
        be.delta_y = event.deltaY;
    }
    else if (event.type == NSEventTypeRightMouseDown) {
        be.is_mouse = 1;
        be.is_right_button = 1;
        be.is_down = 1;
    }
    else if (event.type == NSEventTypeRightMouseUp) {
        be.is_mouse = 1;
        be.is_right_button = 1;
        be.is_up = 1;
    }
    else if (event.type == NSEventTypeRightMouseDragged) {
        be.is_mouse = 1;
        be.is_right_button = 1;
        be.delta_x = event.deltaX;
        be.delta_y = event.deltaY;
    }
    else if (event.type == NSEventTypeMouseMoved) {
        be.is_mouse = 1;
        be.delta_x = event.deltaX;
        be.delta_y = event.deltaY;
    }
    else {
        return;
    }

    front_window_dispatch_event(handle, be);
}

- (void)keyDown:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)keyUp:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)mouseDown:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)mouseDragged:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)mouseUp:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)mouseEntered:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)mouseExited:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)rightMouseDown:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)rightMouseDragged:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)rightMouseUp:(NSEvent *)event {
    [self dispatchEvent:event];
}
- (void)scrollWheel:(NSEvent *)event {
    [self dispatchEvent:event];
}

- (void)windowWillEnterFullScreen:(NSNotification *)notification {
    front_window_will_fullscreen(handle, YES);
}
- (void)windowWillExitFullScreen:(NSNotification *)notification {
    front_window_will_fullscreen(handle, NO);
}
@end

/* global methods */
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

/* window methods */

void *
back_window_init() {
    return [[Window alloc] init];
}

void
back_window_set_handle(void *_window, fat_pointer handle) {
    Window* window = _window;
    [window setHandle:handle];
}

void
back_window_set_title(void *_window, uint8_t const* const title) {
    Window* window = _window;
    NSString* nsTitle = [NSString stringWithUTF8String:(char const*) title];
    [window setTitle: nsTitle];
}

void
back_window_set_needs_layout(void *_window) {
    Window* window = _window;
    NSView* contentView = [window contentView];
    contentView.needsLayout = YES;
}

// should only be called once
void
back_window_set_root(void *_window, void *root_view) {
    Window* window = _window;
    NSView* view = root_view;

    [[window contentView] addSubview: view];
}

void
back_window_set_size(void *_window, double w, double h) {
    Window* window = _window;
    [window setContentSize: NSMakeSize(w, h)];
}

void
back_window_set_min_size(void *_window, double w, double h) {
    Window* window = _window;
    window.contentMinSize = NSMakeSize(w, h);
}

void
back_window_set_max_size(void *_window, double w, double h) {
    Window* window = _window;
    window.contentMaxSize = NSMakeSize(w, h);
}

void
back_window_set_fullscreen(void *_window, uint8_t fs) {
    Window* window = _window;
    if (!!(window.styleMask & NSWindowStyleMaskFullScreen) != fs) {
        [window toggleFullScreen:nil];
    }
}

void
back_window_exit(void *window_p) {
    Window* window = window_p;
    [window close];
}

void
back_window_free(void *_window) {
    Window* window = _window;
    [window release];
}

/* view methods */
void
back_view_clear_children(void *_view) {
    NSView* view = _view;
    while (view.subviews.count > 0) {
        NSView *subview = view.subviews.lastObject;
        [subview removeFromSuperview];
    }
}

void
back_view_remove_child(void *_view, unsigned long long index) {
    NSView* view = _view;
    [view.subviews[index] removeFromSuperview];
}

void
back_view_insert_child(void *_view, void* restrict _child, unsigned long long index) {
    NSView* view = _view;
    NSView* child = _child;

    performing_subview_insertion++;

    if (index == view.subviews.count) {
        [view addSubview:child positioned:NSWindowAbove relativeTo:nil];
    }
    else {
        [view addSubview:child positioned:NSWindowBelow relativeTo:[view.subviews objectAtIndex:index]];
    }

    performing_subview_insertion--;
}

void
back_view_set_frame(void *_view, double left, double top, double width, double height) {
    NSView* view = _view;

    NSRect frame;
    frame.size = NSMakeSize(width, height);
    frame.origin = NSMakePoint(left, top);

    [view setFrame: frame];
}

void
back_free_view(void *view) {
    NSView *nsView = view;
    [nsView release];
}
