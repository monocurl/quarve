#pragma once

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

// box: Box<dyn FnOnce(MSlock) + Send + 'static>
extern void front_execute_box(fat_pointer box);

// box: Box<dyn Fn(ScreenUnit, MSlock)>
extern void front_set_screen_unit_binding(fat_pointer box, double value);

// box: Box<dyn Fn(ScreenUnit, MSlock)>
extern void front_free_screen_unit_binding(fat_pointer box);
