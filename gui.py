import sys
import evdev
from PyQt5 import QtCore, QtGui, QtWidgets
from PyQt5.QtCore import pyqtSlot

from remapper_ui import Ui_RemapperWindow
from add_remap_ui import Ui_NewRemapDialog
import config
import inputdevice
import outputdevice
import util
import remap


def show_message(title, text):
    msg = QtWidgets.QMessageBox()
    msg.setIcon(QtWidgets.QMessageBox.Information)
    msg.setText(text)
    msg.setWindowTitle(title)
    msg.exec()


class DeviceEventWatcher(QtCore.QThread):
    event_log = QtCore.pyqtSignal(object)

    def __init__(self, inputdevice):
        QtCore.QThread.__init__(self)
        self.inputdevice = inputdevice
        self.keep_running = True

    def run(self):
        for event in self.inputdevice.read_loop():
            if not self.keep_running:
                break
            self.event_log.emit(event)


class AddRemapDialog(Ui_NewRemapDialog):

    def __init__(self):
        super().__init__()
        self.dialog = QtWidgets.QDialog()
        self.setupUi(self.dialog)

        self.device = None
        self.device_watcher = None

        self.debug_events = {}
        self.debug_events[evdev.ecodes.EV_ABS] = True
        self.debug_events[evdev.ecodes.EV_CNT] = True
        self.debug_events[evdev.ecodes.EV_FF] = True
        self.debug_events[evdev.ecodes.EV_FF_STATUS] = True
        self.debug_events[evdev.ecodes.EV_KEY] = True
        self.debug_events[evdev.ecodes.EV_LED] = True
        self.debug_events[evdev.ecodes.EV_MAX] = True
        self.debug_events[evdev.ecodes.EV_MSC] = True
        self.debug_events[evdev.ecodes.EV_PWR] = True
        self.debug_events[evdev.ecodes.EV_REL] = True
        self.debug_events[evdev.ecodes.EV_REP] = True
        self.debug_events[evdev.ecodes.EV_SND] = True
        self.debug_events[evdev.ecodes.EV_SW] = True
        self.debug_events[evdev.ecodes.EV_SYN] = True

        self.select_device_button.clicked.connect(self.select_device)
        self.cb_EV_ABS.clicked.connect(self.debug_cb_EV_ABS)
        self.cb_EV_CNT.clicked.connect(self.debug_cb_EV_CNT)
        self.cb_EV_FF.clicked.connect(self.debug_cb_EV_FF)
        self.cb_EV_FF_STATUS.clicked.connect(self.debug_cb_EV_FF_STATUS)
        self.cb_EV_KEY.clicked.connect(self.debug_cb_EV_KEY)
        self.cb_EV_LED.clicked.connect(self.debug_cb_EV_LED)
        self.cb_EV_MAX.clicked.connect(self.debug_cb_EV_MAX)
        self.cb_EV_MSC.clicked.connect(self.debug_cb_EV_MSC)
        self.cb_EV_PWR.clicked.connect(self.debug_cb_EV_PWR)
        self.cb_EV_REL.clicked.connect(self.debug_cb_EV_REL)
        self.cb_EV_REP.clicked.connect(self.debug_cb_EV_REP)
        self.cb_EV_SND.clicked.connect(self.debug_cb_EV_SND)
        self.cb_EV_SW.clicked.connect(self.debug_cb_EV_SW)
        self.cb_EV_SYN.clicked.connect(self.debug_cb_EV_SYN)

        self.refresh_button.clicked.connect(self.refresh)

        self.ev_type_a_cmbx.activated[str].connect(self.refresh_event_a_cmbx)
        self.ev_type_b_cmbx.activated[str].connect(self.refresh_event_b_cmbx)
        for key, ev_type in evdev.ecodes.EV.items():
            self.ev_type_a_cmbx.addItem(ev_type)
            self.ev_type_b_cmbx.addItem(ev_type)

        self.refresh_event_a_cmbx(self.ev_type_a_cmbx.currentText())
        self.refresh_event_b_cmbx(self.ev_type_b_cmbx.currentText())

        header = self.event_table.horizontalHeader()
        header.setSectionResizeMode(0, QtWidgets.QHeaderView.Stretch)
        header.setSectionResizeMode(1, QtWidgets.QHeaderView.Stretch)
        header.setSectionResizeMode(2, QtWidgets.QHeaderView.Stretch)
        header.setSectionResizeMode(3, QtWidgets.QHeaderView.Stretch)
        self.event_table.setEditTriggers(QtWidgets.QTableWidget.NoEditTriggers)

        self.add_event_button.clicked.connect(self.add_event)
        self.remove_event_button.clicked.connect(self.remove_event)

        self.dialog_buttons.accepted.connect(self.ok_button)

        self.preset_cmbx.activated[str].connect(self.refresh_preset)
        self.preset_cmbx.addItem("none")
        self.preset_cmbx.addItem("copy")
        for preset in util.uinput_presets():
            self.preset_cmbx.addItem(preset['name'])

        for bus_type in util.bus_types():
            self.bustype_cmbx.addItem(evdev.ecodes.BUS[bus_type])
        self.bustype_cmbx.setCurrentIndex(2)

    def refresh_preset(self, text):
        if text == 'none':
            self.vendor_field.setText("")
            self.product_field.setText("")
            self.version_field.setText("")
        if text == 'copy':
            if self.device is None:
                show_message("no device selected",
                             "No device selected! Cannot copy information!")
            else:
                self.vendor_field.setText(str(self.device.vendor()))
                self.product_field.setText(str(self.device.product()))
                self.version_field.setText(str(self.device.version()))
        else:
            for preset in util.uinput_presets():
                if preset['name'] == text:
                    self.vendor_field.setText(str(preset['vendor']))
                    self.product_field.setText(str(preset['product']))
                    self.version_field.setText(str(preset['version']))
                    break

    def collect_capabilities(self):
        ev_names = {}
        for key, val in evdev.ecodes.EV.items():
            ev_names[val] = key

        capabilities = self.device.capabilities()
        for entry in range(self.event_table.rowCount()):
            event_b = self.event_table.item(entry, 2).text()
            code_b = self.event_table.item(entry, 3).text()
            capabilities[ev_names[event_b]] = util.value_to_ecode(code_b)

        capabilities.pop(0, None)
        return capabilities

    def collect_event_map(self):
        event_map = {}
        for entry in range(self.event_table.rowCount()):
            ex_event_a = self.event_table.item(entry, 0).text()
            ex_code_a = self.event_table.item(entry, 1).text()
            ex_event_b = self.event_table.item(entry, 2).text()
            ex_code_b = self.event_table.item(entry, 3).text()

            src = util.value_to_ecode(ex_code_a)
            dest = util.value_to_ecode(ex_code_b)
            event_map[src] = dest

        return event_map

    def validate(self):
        name = self.remap_name_field.text()
        if name is None:
            return False
        if name.strip() == "":
            return False
        return True

    def ok_button(self):

        if not self.validate():
            show_message(
                "Invalid Input", "Invalid input detected! Please check all fields! (name must be set!!)")
            return

        name = self.remap_name_field.text()
        capabilities = self.collect_capabilities()
        event_map = self.collect_event_map()

        for key, val in evdev.ecodes.BUS.items():
            if val == self.bustype_cmbx.currentText():
                bustype = key

        vendor = int(self.vendor_field.text())
        product = int(self.product_field.text())
        version = int(self.version_field.text())

        grab = self.grab_cb.checkState()
        print(grab)

        out_dev = outputdevice.OutputDevice(
            name, capabilities, bustype, vendor, product, version)
        remapper = remap.Remap(event_map, self.device, out_dev, grab)
        cfg = config.Config()
        cfg.add_remapper(remapper)
        cfg.save()

    def add_event(self):
        event_a = self.ev_type_a_cmbx.currentText()
        code_a = self.event_a_cmbx.currentText()
        event_b = self.ev_type_a_cmbx.currentText()
        code_b = self.event_b_cmbx.currentText()

        for entry in range(self.event_table.rowCount()):
            ex_event_a = self.event_table.item(entry, 0).text()
            ex_code_a = self.event_table.item(entry, 1).text()
            ex_event_b = self.event_table.item(entry, 2).text()
            ex_code_b = self.event_table.item(entry, 3).text()

            if event_a == ex_event_a and code_a == ex_code_a and event_b == ex_event_b and code_b == ex_code_b:
                print("Item already exists!")
                return

        row_position = self.event_table.rowCount()
        self.event_table.insertRow(row_position)
        self.event_table.setItem(
            row_position, 0, QtWidgets.QTableWidgetItem(event_a))
        self.event_table.setItem(
            row_position, 1, QtWidgets.QTableWidgetItem(code_a))
        self.event_table.setItem(
            row_position, 2, QtWidgets.QTableWidgetItem(event_b))
        self.event_table.setItem(
            row_position, 3, QtWidgets.QTableWidgetItem(code_b))

    def remove_event(self):
        indexes = self.event_table.selectionModel().selectedRows()
        for index in indexes:
            self.event_table.removeRow(index.row())

    def refresh_event_a_cmbx(self, text):
        codes_for_ev = util.ev_to_codes(evdev.ecodes.ecodes[text])
        self.event_a_cmbx.clear()
        for key, ev_type in codes_for_ev.items():
            if type(ev_type) is list:
                for entry in ev_type:
                    self.event_a_cmbx.addItem(entry)
            else:
                self.event_a_cmbx.addItem(ev_type)

    def refresh_event_b_cmbx(self, text):
        codes_for_ev = util.ev_to_codes(evdev.ecodes.ecodes[text])
        self.event_b_cmbx.clear()
        for key, ev_type in codes_for_ev.items():
            if type(ev_type) is list:
                for entry in ev_type:
                    self.event_b_cmbx.addItem(entry)
            else:
                self.event_b_cmbx.addItem(ev_type)

    def refresh(self):
        self.device_list.clear()
        self.devices = inputdevice.list_devices()
        for device in self.devices:
            self.device_list.addItem(device.info_string())

    def select_device(self):
        if self.device is not None and self.device_watcher is not None:
            self.device_watcher.keep_running = False

        indexes = self.device_list.selectionModel().selectedRows()
        for index in indexes:
            self.device = self.devices[index.row()]
            if self.preset_cmbx.currentText() == 'copy':
                self.refresh_preset("copy")

        if self.device is not None:
            self.device_watcher = DeviceEventWatcher(self.device)
            self.device_watcher.event_log.connect(self.update_debug_event_log)
            self.device_watcher.start()

    def update_debug_event_log(self, data):
        if data.type in self.debug_events:
            if self.debug_events[data.type]:
                print(data)
                event_type = evdev.ecodes.EV[data.type]

                codes = util.ev_to_codes(data.type)
                if data.code in codes:
                    event_code = util.ev_to_codes(data.type)[data.code]
                else:
                    event_code = str(data.code)
                event_value = str(data.value)
                event_str = "type=" + event_type + "\tcode=" + \
                    event_code + "\tvalue" + event_value

                self.debug_area.insertPlainText(str(event_str) + "\n")
                self.debug_area.moveCursor(QtGui.QTextCursor.End)

    def show(self):
        self.refresh()
        self.dialog.exec_()

    def debug_cb_EV_ABS(self, val):
        self.debug_events[evdev.ecodes.EV_ABS] = val

    def debug_cb_EV_CNT(self, val):
        self.debug_events[evdev.ecodes.EV_CNT] = val

    def debug_cb_EV_FF(self, val):
        self.debug_events[evdev.ecodes.EV_FF] = val

    def debug_cb_EV_FF_STATUS(self, val):
        self.debug_events[evdev.ecodes.EV_FF_STATUS] = val

    def debug_cb_EV_KEY(self, val):
        self.debug_events[evdev.ecodes.EV_KEY] = val

    def debug_cb_EV_LED(self, val):
        self.debug_events[evdev.ecodes.EV_LED] = val

    def debug_cb_EV_MAX(self, val):
        self.debug_events[evdev.ecodes.EV_MAX] = val

    def debug_cb_EV_MSC(self, val):
        self.debug_events[evdev.ecodes.EV_MSC] = val

    def debug_cb_EV_PWR(self, val):
        self.debug_events[evdev.ecodes.EV_PWR] = val

    def debug_cb_EV_REL(self, val):
        self.debug_events[evdev.ecodes.EV_REL] = val

    def debug_cb_EV_REP(self, val):
        self.debug_events[evdev.ecodes.EV_REP] = val

    def debug_cb_EV_SND(self, val):
        self.debug_events[evdev.ecodes.EV_SND] = val

    def debug_cb_EV_SW(self, val):
        self.debug_events[evdev.ecodes.EV_SW] = val

    def debug_cb_EV_SYN(self, val):
        self.debug_events[evdev.ecodes.EV_SYN] = val


class GUI(Ui_RemapperWindow):
    def __init__(self):
        super().__init__()

    def setupUi(self, window):
        super().setupUi(window)

        self.add_button.clicked.connect(self.add_remapper_button)
        self.start_stop_button.clicked.connect(self.start_stop_remapper_button)
        self.remove_button.clicked.connect(self.remove_remapper)

        header = self.remap_table.horizontalHeader()
        header.setSectionResizeMode(0, QtWidgets.QHeaderView.Stretch)
        header.setSectionResizeMode(1, QtWidgets.QHeaderView.ResizeToContents)
        self.remap_table.setEditTriggers(QtWidgets.QTableWidget.NoEditTriggers)

        self.remappers = config.Config().get_remappers()
        self.refresh_table()

    def refresh_table(self):
        self.remap_table.clear()
        for remapper in self.remappers:
            row_position = self.remap_table.rowCount()
            self.remap_table.insertRow(row_position)
            self.remap_table.setItem(
                row_position, 0, QtWidgets.QTableWidgetItem(remapper.outputdevice.name))
            if remapper.inputdevice == None:
                self.remap_table.setItem(
                    row_position, 1, QtWidgets.QTableWidgetItem("not available"))
            else:
                self.remap_table.setItem(
                    row_position, 1, QtWidgets.QTableWidgetItem("stopped"))

    def add_remapper_button(self):
        dialog = AddRemapDialog()
        dialog.show()
        self.refresh_table()

    def start_stop_remapper_button(self):
        indexes = self.remap_table.selectionModel().selectedRows()
        for index in indexes:
            state = self.remap_table.item(index.row(), 1).text()
            if state == "not available":
                show_message("Device not available", "The selected device is not available. Is it connected? Did the port change?")
                continue
            elif state == "stopped":
                self.remappers[index.row()].start()
                self.remap_table.setItem(
                    index.row(), 1, QtWidgets.QTableWidgetItem("running"))
            elif state == "running":
                self.remappers[index.row()].stop()
                self.remap_table.setItem(
                    index.row(), 1, QtWidgets.QTableWidgetItem("stopped"))

    def remove_remapper(self):
        indexes = self.remap_table.selectionModel().selectedRows()
        for index in indexes:
            remapper = self.remappers[index.row()]
            remapper.stop()
            config.Config().remove_remapper(remapper)
            self.remap_table.removeRow(index.row())


def show():
    app = QtWidgets.QApplication(sys.argv)
    window = QtWidgets.QMainWindow()
    ui = GUI()
    ui.setupUi(window)
    window.show()

    sys.exit(app.exec_())
