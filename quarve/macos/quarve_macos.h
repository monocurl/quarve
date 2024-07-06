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

typedef struct size {
    double w, h;
} size;