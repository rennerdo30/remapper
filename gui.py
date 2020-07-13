import sys
import random
from PySide2.QtWidgets import (QApplication, QLabel, QPushButton,
                               QVBoxLayout, QWidget, QListView)
from PySide2.QtCore import Slot, Qt

import evdev

class MyWidget(QWidget):
    def __init__(self):
        QWidget.__init__(self)

        self.device_list = QListView()

        self.layout = QVBoxLayout()
        self.layout.addWidget(self.device_list)
        self.setLayout(self.layout)


    @Slot()
    def magic(self):
        self.text.setText(random.choice(self.hello))
