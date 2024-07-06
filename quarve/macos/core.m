#import <Cocoa/Cocoa.h>
#import "color.h"
#import "quarve_macos.h"

/* front end */
extern void front_will_spawn(void);

// fp: &'static dyn WindowBase
extern bool front_window_should_close(fat_pointer p);

// fp: &'static dyn WindowBase
extern void front_window_layout(fat_pointer p, double w, double h);

// fp: &'static dyn WindowBase
extern void front_window_dispatch_event(fat_pointer handle, buffer_event event);

// fp: &'static dyn WindowBase
extern void front_window_will_fullscreen(fat_pointer p, uint8_t fs);

// box: &dyn FnOnce(MSlock) + Send + 'static
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

@interface ContentView : NSView
- (void)layout;
@end

@interface Window : NSWindow<NSWindowDelegate> {
    /* callbacks */
    @public fat_pointer handle;
}
@end

@implementation ContentView
- (void)layout {
    [super layout];

    Window* window = (Window*) self.window;
    front_window_layout(window->handle, (double) NSWidth(window.frame), (double) NSHeight(window.frame));
}
@end

@implementation Window
- (instancetype)init {
    [super initWithContentRect:NSMakeRect(0, 0, 2, 2) styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
    [self setIsVisible:YES];

    self.delegate = self;
    self.releasedWhenClosed = NO;

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
- (void)sendEvent:(NSEvent*)event {
    buffer_event be = { .native_event = event };

    be.cursor_x = event.locationInWindow.x;
    be.cursor_y = event.locationInWindow.y;
    if (be.cursor_y > [self contentRectForFrameRect: self.frame].size.height) {
        [super sendEvent: event];
        return;
    }

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
        [super sendEvent: event];
        return;
    }

    front_window_dispatch_event(handle, be);
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
void *
back_view_layout_init() {
    return [[NSView alloc] init];
}

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
    if (index == view.subviews.count) {
        [view addSubview:child positioned:NSWindowAbove relativeTo:nil];
    }
    else {
        [view addSubview:child positioned:NSWindowBelow relativeTo:[view.subviews objectAtIndex:index]];
    }
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

void *
back_view_layer_init() {
    NSView* ret = [[NSView alloc] init];
    [ret setWantsLayer: YES];
    return ret;
}

void
back_view_layer_update(void *_view, color background_color, color border_color, double corner_radius, double border_width, float opacity)
{
    NSView* view = _view;
    view.layer.borderWidth = border_width;
    view.layer.cornerRadius = corner_radius;
    view.layer.backgroundColor = color_to_cg_color(background_color);
    view.layer.borderColor = color_to_cg_color(border_color);
    view.layer.opacity = opacity;
}

void *
back_view_image_init(uint8_t const* path)
{
    NSString* const nsPath = [NSString stringWithUTF8String:(char const*) path];
    NSImage* const image = [[NSImage alloc] initByReferencingFile:nsPath];
    if (!image || !image.valid) {
        return NULL;
    }

    NSImageView *const imageView = [[NSImageView alloc] init];
    imageView.image = image;
    imageView.imageScaling = NSImageScaleProportionallyUpOrDown;
    return imageView;
}

size
back_view_image_size(void *_image)
{
    NSImageView* const image = _image;
    NSSize const intrinsic = image.intrinsicContentSize;

    return (size) { (double) intrinsic.width, (double) intrinsic.height };
}