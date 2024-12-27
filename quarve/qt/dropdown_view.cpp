#include <QComboBox>
#include "../inc/util.h"
#include "front.h"

class Dropdown : public QComboBox {
public:
    Dropdown(fat_pointer binding) :
          binding(binding)
        , in_transaction(false)
    {
        connect(this, QOverload<int>::of(&QComboBox::currentIndexChanged),
            [this](int) {
                if (!in_transaction) {
                    QString currentText = this->currentText();
                    if (currentIndex() == -1) {
                        front_set_opt_string_binding(this->binding, nullptr);
                    } else {
                        QByteArray utf8Text = currentText.toUtf8();
                        front_set_opt_string_binding(this->binding,
                            reinterpret_cast<uint8_t*>(utf8Text.data()));
                    }
                }
            });
    }

    ~Dropdown()
    {
        front_free_opt_string_binding(binding);
    }

    void addOption(const QString& option)
    {
        addItem(option);
    }

    uint8_t selectOption(const QString& selection)
    {
        int index = findText(selection);
        if (index != -1) {
            setCurrentIndex(index);
            return 0;
        }
        return 1;
    }

    fat_pointer binding;
    bool in_transaction;
};

extern "C" void*
back_view_dropdown_init(fat_pointer binding)
{
    return new Dropdown(binding);
}

extern "C" void
back_view_dropdown_add(void *_view, unsigned char const* option)
{
    Dropdown* dd = static_cast<Dropdown*>(_view);
    QString optionString = QString::fromUtf8(reinterpret_cast<const char*>(option));
    dd->in_transaction = true;
    dd->addOption(optionString);
    dd->in_transaction = false;
}

extern "C" uint8_t
back_view_dropdown_select(void *_view, unsigned char const* selection)
{
    Dropdown* dd = static_cast<Dropdown*>(_view);
    if (selection) {
        dd->in_transaction = true;
        QString selectionString =
            QString::fromUtf8(reinterpret_cast<const char*>(selection));
        uint8_t ret = dd->selectOption(selectionString);
        dd->in_transaction = false;
        return ret;
    } else {
        dd->in_transaction = true;
        dd->setCurrentIndex(-1);
        dd->in_transaction = false;
        return 0;
    }
}

extern "C" void
back_view_dropdown_clear(void* _view)
{
    Dropdown* dd = static_cast<Dropdown*>(_view);
    dd->clear();
}

extern "C" size
back_view_dropdown_size(void *_view)
{
    Dropdown* dd = static_cast<Dropdown*>(_view);
    QSize sizeHint = dd->sizeHint();
    return (size) {
        static_cast<double>(sizeHint.width()),
        static_cast<double>(sizeHint.height())
    };
}