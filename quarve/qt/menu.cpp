#include "../inc/util.h"
#include "front.h"

extern "C" void*
back_menu_init(uint8_t const* title)
{
    return nullptr;
}

extern "C" void
back_menu_add(void *_menu, void* _item) {
}

extern "C" void
back_menu_free(void *_menu) {
}

// button
extern "C" void*
back_menu_separator_init()
{
    return nullptr;
}

extern "C" void
back_menu_separator_free(void* _separator)
{
}

extern "C" void*
back_menu_button_init(uint8_t const* title, uint8_t const* keyEquivalent, uint8_t modifiers)
{
    return nullptr;
}

extern "C" void
back_menu_button_set_title(void* _button, uint8_t const* title)
{

}

extern "C" void
back_menu_button_set_action(void *_button, fat_pointer action)
{

}

extern "C" void
back_menu_button_set_enabled(void *_button, uint8_t enabled)
{

}

extern "C" void
back_menu_button_set_submenu(void *_button, void* _menu)
{

}

extern "C" void
back_menu_button_free(void *_button)
{

}