#include <QWidget>
#include <QMessageBox>

#include "../inc/util.h"
#include "front.h"

extern "C" void*
back_message_box_init(uint8_t const* title, uint8_t const* message)
{
    QMessageBox* msgBox = new QMessageBox();

    if (title) {
        QString qTitle = QString::fromUtf8(reinterpret_cast<const char*>(title));
        msgBox->setWindowTitle(qTitle);
    }

    if (message) {
        QString qMessage = QString::fromUtf8(reinterpret_cast<const char*>(message));
        msgBox->setText(qMessage);
    }

    return msgBox;
}

extern "C" void
back_message_box_add_button(void *mb, uint8_t button_type)
{
    QMessageBox* msgBox = (QMessageBox*) (mb);
    switch (button_type) {
        case BUTTON_TYPE_OK:
            msgBox->addButton(QMessageBox::Ok);
            break;

        case BUTTON_TYPE_CANCEL:
            msgBox->addButton(QMessageBox::Cancel);
            break;

        case BUTTON_TYPE_DELETE:
            msgBox->addButton("Delete", QMessageBox::DestructiveRole);
            break;
    }
}

// returns index that was clicked
extern "C" int
back_message_box_run(void *mb)
{
    QMessageBox* msgBox = (QMessageBox*) mb;

    int result = msgBox->exec();

    QList<QAbstractButton*> buttons = msgBox->buttons();
    QAbstractButton* clickedButton = msgBox->clickedButton();
    int index = buttons.indexOf(clickedButton);

    delete msgBox;

    return (index >= 0) ? index : 0;
}