#pragma once

#import "util.h"

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

// box: Box<dyn FnOnce(MainThreadMarker) + Send + 'static>
extern void front_execute_fn_once(fat_pointer box);

// box: Box<dyn FnMut(MSlock)>
extern void front_execute_fn_mut(fat_pointer box);

// box: Box<dyn FnMut(MSlock)>
extern void front_free_fn_mut(fat_pointer box);

// box: Box<dyn Fn(ScreenUnit, MSlock)>
extern void front_set_screen_unit_binding(fat_pointer box, double value);

// box: Box<dyn Fn(ScreenUnit, MSlock)>
extern void front_free_screen_unit_binding(fat_pointer box);

// box: Box<dyn Fn(*const u8, MSlock)>
// NOTE: can also be a string_binding
extern void front_set_opt_string_binding(fat_pointer box, uint8_t const* value);

// box: Box<dyn Fn(*const u8, MSlock)>
// NOTE: can also be a string_binding
extern void front_free_opt_string_binding(fat_pointer box);

// box: Box<dyn Fn(bool, i32, MSlock)>
extern void front_set_token_binding(fat_pointer box, uint8_t has_value, int32_t value);

// box: Box<dyn Fn(bool, i32, MSlock)>
extern void front_free_token_binding(fat_pointer box);

// box: Box<dyn Fn(u8, MSlock)>
extern void front_set_bool_binding(fat_pointer box, uint8_t value);

// box: Box<dyn Fn(u8, MSlock)>
extern void front_free_bool_binding(fat_pointer box);

// box is a page store container
extern void front_replace_textview_range(fat_pointer box, size_t start, size_t end, uint8_t const* value);

// box is a page store container
extern void front_set_textview_selection(fat_pointer box, size_t start, size_t len);

// box is a page store container
extern void front_free_textview_state(fat_pointer box);

// box: Box<dyn FnMut(keycode, MSlock)> -> bool
// key code: 0 -> tab
//           1 -> un_tab
//           2 -> newline
//           3 -> alt new line
//           4 -> escape
//           5 -> left 6 -> right 7 -> down 8 -> up
extern uint8_t front_execute_key_callback(fat_pointer box, size_t key_code);

// box: Box<dyn FnMut(keycode, MSlock)> -> bool
extern void front_free_key_callback(fat_pointer box);

