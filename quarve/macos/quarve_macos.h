#pragma once

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
    EVENT_MODIFIER_COMMAND = 1,
    EVENT_MODIFIER_CONTROL = 2,
    EVENT_MODIFIER_SHIFT   = 4,
    EVENT_MODIFIER_FN      = 8,
    EVENT_MODIFIER_ALT_OPTION = 16
};

typedef struct size {
    double w, h;
} size;
