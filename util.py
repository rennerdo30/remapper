import json
import evdev

import inputdevice


def str2bool(v):
    return v.lower() in ("y", "yes", "true", "t", "1")


def ecode_to_value(ecode):
    if ecode is None:
        return None
    elif str(ecode).isnumeric():
        value = int(ecode)
        for key, code in evdev.ecodes.ecodes.items():
            if value == code:
                return key
    else:
        if ecode.upper() in evdev.ecodes.ecodes:
            return ecode.upper()
    return None


def value_to_ecode(value):
    if value is None:
        return None
    elif str(value).isnumeric():
        value = int(value)
        for key, code in evdev.ecodes.ecodes.items():
            if value == code:
                return code
    else:
        if value.upper() in evdev.ecodes.ecodes:
            return evdev.ecodes.ecodes[value.upper()]
    return None


def ev_to_codes(ev):
    if ev == evdev.ecodes.EV_SYN:
        return evdev.ecodes.SYN
    if ev == evdev.ecodes.EV_KEY:
        return evdev.ecodes.KEY
    if ev == evdev.ecodes.EV_REL:
        return evdev.ecodes.REL
    if ev == evdev.ecodes.EV_ABS:
        return evdev.ecodes.ABS
    if ev == evdev.ecodes.EV_MSC:
        return evdev.ecodes.MSC
    if ev == evdev.ecodes.EV_SW:
        return evdev.ecodes.SW
    if ev == evdev.ecodes.EV_LED:
        return evdev.ecodes.LED
    if ev == evdev.ecodes.EV_SND:
        return evdev.ecodes.SND
    if ev == evdev.ecodes.EV_REP:
        return evdev.ecodes.REP
    if ev == evdev.ecodes.EV_FF:
        return evdev.ecodes.FF
    if ev == evdev.ecodes.EV_PWR:
        return {}
    if ev == evdev.ecodes.EV_FF_STATUS:
        return evdev.ecodes.FF_STATUS
    if ev == evdev.ecodes.EV_MAX:
        return {}
    if ev == evdev.ecodes.EV_CNT:
        return {}
    if ev == evdev.ecodes.EV_UINPUT:
        return {}
    if ev == evdev.ecodes.EV_VERSION:
        return {}


def bus_types():
    types = [
        evdev.ecodes.BUS_PCI,
        evdev.ecodes.BUS_ISAPNP,
        evdev.ecodes.BUS_USB,
        evdev.ecodes.BUS_HIL,
        evdev.ecodes.BUS_BLUETOOTH,
        evdev.ecodes.BUS_VIRTUAL,
        evdev.ecodes.BUS_ISA,
        evdev.ecodes.BUS_I8042,
        evdev.ecodes.BUS_XTKBD,
        evdev.ecodes.BUS_RS232,
        evdev.ecodes.BUS_GAMEPORT,
        evdev.ecodes.BUS_PARPORT,
        evdev.ecodes.BUS_AMIGA,
        evdev.ecodes.BUS_ADB,
        evdev.ecodes.BUS_I2C,
        evdev.ecodes.BUS_HOST,
        evdev.ecodes.BUS_GSC,
        evdev.ecodes.BUS_ATARI,
        evdev.ecodes.BUS_SPI,
        evdev.ecodes.BUS_RMI,
        evdev.ecodes.BUS_CEC,
        evdev.ecodes.BUS_INTEL_ISHTP
    ]
    return types


def uinput_presets():
    presets = []
    presets.append({"name": "Xbox Gamepad", "vendor": 1118,
                    "product": 654, "version": 272})

    return presets


def translate(value, leftMin, leftMax, rightMin, rightMax):
    leftSpan = leftMax - leftMin
    rightSpan = rightMax - rightMin
    valueScaled = float(value - leftMin) / float(leftSpan)
    return rightMin + (valueScaled * rightSpan)


def select_evdev_via_console():
    print("The following devices are available:")
    devices = [evdev.InputDevice(path) for path in evdev.list_devices()]
    counter = 0
    for device in devices:
        input_device = inputdevice.InputDevice(device)
        print("[", str(counter), "]", input_device.info_string())
        counter = counter + 1

    print("Select the device to remap by entering the number in front of the desired entry.")
    entered_idx = input()

    valid_input = False
    device = None
    while not valid_input:
        if entered_idx.isnumeric():
            input_idx = int(entered_idx)
            if input_idx < 0 or input_idx > len(devices) - 1:
                valid_input = False
                print("Input was not valid. Please try again!")
                entered_idx = input()
            else:
                valid_input = True
                entered_idx = input_idx

                device = inputdevice.InputDevice(devices[entered_idx])
                print("Selected device:", device.info_string())
                print("Do you want to continue? [y/n]")
                confirm = input()
                if confirm == 'y':
                    break
                else:
                    print("Enter new index:")
                    entered_idx = input()
                    valid_input = False
                    continue
    return device

