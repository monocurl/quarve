#import "../inc/util.h"
#import "../inc/front.h"

extern "C" void*
back_message_box_init(uint8_t const* title, uint8_t const* message)
{

}

extern "C" void
back_message_box_add_button(void *mb, uint8_t button_type)
{

}

// returns index that was clicked
extern "C" int
back_message_box_run(void *mb)
{
    return 0;
}