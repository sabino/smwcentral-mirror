#include <QApplication>
#include <QLabel>

int main(int argc, char **argv) {
  QApplication app(argc, argv);
  QLabel label("smwapt GUI scaffold");
  label.resize(480, 160);
  label.show();
  return app.exec();
}
