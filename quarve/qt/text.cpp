#include "../inc/util.h"
#include "color.h"
#include "../inc/front.h"

extern "C" void*
back_text_init()
{
    return nullptr;
}

extern "C" void
back_text_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
) {

}

extern "C" size
back_text_size(void* view, size suggested)
{
    return (size) { 0, 0 };
}

// MARK: textfield

extern "C" void*
back_text_field_init(
    fat_pointer text_binding,
    fat_pointer focused_binding,
    fat_pointer callback,
    int32_t token,
    uint8_t unstyled,
    uint8_t secure
)
{
    return nullptr;
}

extern "C" void
back_text_field_focus(void *view)
{

}

extern "C" void
back_text_field_unfocus(void *view)
{

}

extern "C" void
back_text_field_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
)
{

}

extern "C" size
back_text_field_size(void* view, size suggested)
{
    return (size) { 0, 0 };
}

extern "C" void
back_text_field_select_all(void *view)
{

}

extern "C" void
back_text_field_cut(void *view)
{

}

extern "C" void
back_text_field_copy(void *view)
{

}

extern "C" void
back_text_field_paste(void *view)
{

}

// MARK: textview
extern "C" void *
back_text_view_init()
{
    return nullptr;
}

// may discard attributes
extern "C" void
back_text_view_full_replace(
    void *tv,
    const uint8_t* with,
    fat_pointer _state,
    fat_pointer selected,
    fat_pointer key_callback
)
{

}

extern "C" void
back_text_view_replace(void *tv, size_t start, size_t len, const uint8_t* with)
{

}

extern "C" void
back_text_view_set_font(
    void *tv, uint8_t const* font_path, double font_size
)
{

}

extern "C" void
back_text_view_set_editing_state(void *tv, uint8_t editing)
{

}

extern "C" void
back_text_view_set_line_attributes(
    void *tv,
    size_t line_no, size_t start, size_t end,
    int justification_sign,
    double leading_indentation, double trailing_indentation
)
{

}

extern "C" void
back_text_view_set_char_attributes(
    void *tv, size_t start, size_t end,
    uint8_t bold, uint8_t italic, uint8_t underline, uint8_t strikethrough,
    color back, color front
)
{

}

extern "C" void
back_text_view_set_selection(void *tv, size_t start, size_t len)
{

}

extern "C" void
back_text_view_get_selection(void *tv, size_t *start, size_t* end)
{

}

extern "C" double
back_text_view_get_line_height(void *tv, size_t line, size_t start, size_t end, double width)
{
    return 0.0;
}

extern "C" void
back_text_view_get_cursor_pos(void *tv, double *x, double *y)
{

}


extern "C" void
back_text_view_set_page_id(void *tv, int32_t page_id)
{

}

extern "C" void
back_text_view_focus(void *tv)
{

}

extern "C" void
back_text_view_unfocus(void *tv)
{

}

extern "C" void
back_text_view_copy(void *tv)
{

}

extern "C" void
back_text_view_cut(void *tv)
{

}

extern "C" void
back_text_view_paste(void *tv)
{

}

extern "C" void
back_text_view_select_all(void *tv)
{

}