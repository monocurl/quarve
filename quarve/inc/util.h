#pragma once

#include <stdint.h>
#include <stdlib.h>

#define EPSILON 1e-4

// quarve/view/modal.rs
enum button_type {
    BUTTON_TYPE_OK = 1,
    BUTTON_TYPE_CANCEL = 2,
    BUTTON_TYPE_DELETE = 3,
};

// quarve/native.rs
enum textview_callback {
    TEXTVIEW_CALLBACK_KEYCODE_TAB = 0,
    TEXTVIEW_CALLBACK_KEYCODE_UNTAB = 1,
    TEXTVIEW_CALLBACK_KEYCODE_NEWLINE = 2,
    TEXTVIEW_CALLBACK_KEYCODE_ALT_NEWLINE = 3,
    TEXTVIEW_CALLBACK_KEYCODE_ESCAPE = 4,
    TEXTVIEW_CALLBACK_KEYCODE_LEFT = 5,
    TEXTVIEW_CALLBACK_KEYCODE_RIGHT = 6,
    TEXTVIEW_CALLBACK_KEYCODE_DOWN = 7,
    TEXTVIEW_CALLBACK_KEYCODE_UP = 8,
};

typedef struct fat_pointer {
    void const *p0;
    void const *p1;
} fat_pointer;

typedef struct buffer_event {
    uint8_t is_mouse;
    uint8_t is_scroll;
    uint8_t is_up;
    uint8_t is_down;
    uint8_t is_left_button;
    uint8_t is_right_button;
    uint8_t modifiers;
    double cursor_x;
    double cursor_y;
    // scroll or mouse delta
    double delta_x;
    double delta_y;
    unsigned char const* key_characters;
    void *native_event;
} buffer_event;

// must match rust definition
enum event_modifiers {
    EVENT_MODIFIER_CONTROL = 1,
    EVENT_MODIFIER_META = 2,
    EVENT_MODIFIER_SHIFT   = 4,
    EVENT_MODIFIER_FN      = 8,
    EVENT_MODIFIER_ALT_OPTION = 16
};

// matches modifiers/Cursor
enum cursor {
    CURSOR_ARROW  = 0,
    CURSOR_POINTER= 1,
    CURSOR_IBEAM  = 2,
    CURSOR_HORIZONTAL_RESIZE = 3,
    CURSOR_VERTICAL_RESIZE = 4,
};

typedef struct size {
    double w, h;
} size;
