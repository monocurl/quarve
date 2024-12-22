#import "../util.h"
#import "../inc/front.h"

void*
back_message_box_init(uint8_t const* title, uint8_t const* message)
{
    NSAlert* alert = [[NSAlert alloc] init];
    if (title) {
        NSString* const nsTitle = [NSString stringWithUTF8String:(char const*) title];
        alert.messageText = nsTitle;
    }

    if (message) {
        NSString* const nsMessage = [NSString stringWithUTF8String:(char const*) message];
        alert.informativeText = nsMessage;
    }

    return alert;
}

void
back_message_box_add_button(void *mb, uint8_t button_type)
{
    NSAlert* alert = mb;
    switch (button_type) {
    case BUTTON_TYPE_OK:
        [alert addButtonWithTitle: @"OK"];
        break;
    case BUTTON_TYPE_CANCEL:
        [alert addButtonWithTitle: @"Cancel"];
        break;
    case BUTTON_TYPE_DELETE:
        [alert addButtonWithTitle: @"Delete"].hasDestructiveAction = YES;
        break;
    }
}

// returns index that was clicked
int
back_message_box_run(void *mb)
{
    NSAlert* alert = mb;
    NSModalResponse response = [alert runModal];
    [alert release];

    switch (response) {
    case NSAlertFirstButtonReturn:
        return 0;
    case NSAlertSecondButtonReturn:
        return 1;
    case NSAlertThirdButtonReturn:
        return 2;
    default:
        //
        return 0;
    }
}