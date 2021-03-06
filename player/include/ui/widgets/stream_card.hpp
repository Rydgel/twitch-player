#pragma once

#include <QWidget>

#include "api/twitch.hpp"

namespace Ui {
    class StreamCard;
}

class StreamCard: public QWidget {
    Q_OBJECT

public:
    StreamCard(StreamData, QWidget * = nullptr);

protected:
    void mousePressEvent(QMouseEvent *) override;

signals:
    void clicked(QString);

private:
    std::unique_ptr<Ui::StreamCard> _ui;

    StreamData _data;
};
