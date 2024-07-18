#import <Cocoa/Cocoa.h>
#import "util.h"
#import "front.h"

// quarve/view/modal.rs
enum button_type {
    BUTTON_TYPE_OK = 1,
    BUTTON_TYPE_CANCEL = 2,
};

void*
back_message_box_init(uint8_t const* title, uint8_t const* message)
{
    NSAlert* alert = [[NSAlert alloc] init];
    if (title) {

    }

    if (message) {

    }

    return alert;
}

void
back_message_box_add_button(void *mb, uint8_t button_type)
{
    NSAlert* alert = mb;
    switch (button_type) {
    case BUTTON_TYPE_OK:
        break;
    case BUTTON_TYPE_CANCEL:
        break;
    }
}

// returns index that was clicked
int
back_message_box_run(void *mb)
{
    NSAlert* alert = mb;
    NSModalResponse response = [alert runModal];

    switch (response) {
    case NSAlertFirstButtonReturn:
        return 0;
    case NSAlertSecondButtonReturn:
        return 1;
    case NSAlertThirdButtonReturn;
        return 2;
    default:
        return 2 + response - NSAlertThirdButtonReturn;
    }
}

void
back_message_box_free(void* mb)
{
    NSAlert* alert = mb;
    [alert release];
}