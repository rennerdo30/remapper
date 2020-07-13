#!/usr/bin/python3
import evdev
import json
import sys
import argparse
from PySide2.QtWidgets import QApplication

import outputdevice
import inputdevice
import remap
import util
import config
import gui

CONFIG = config.Config()


def main():
    print("You have the following options:")
    print("1.) create\t\tcreates a new remap")
    print("2.) edit\t\tedit a remap")
    print("3.) delete\t\tdelete a remap")
    print("4.) run\t\tjust runs the remapper")

    action = input().strip().lower()
    if action == "1" or action == "create":
        create()
    elif action == "2" or action == "edit":
        edit()
    elif action == "3" or action == "delete":
        delete()
    elif action == "4" or action == "run":
        run()


def create():
    device = util.select_evdev_via_console()

    print("We are now starting to remap buttons.")
    print("Two modes are available:")
    print("1.) interactive (unavailable)")
    print("2.) manual")
    selected_mode = input()
    if selected_mode == '1':
        interactive_mode(device)
    elif selected_mode == '2':
        manual_mode(device)
    else:
        print("wrong input!")
        exit(-1)


def interactive_mode(device):
    print("not implemented yet!")


def manual_mode(device):
    print("You've selected the manual mode for remap creation!")
    print("You woll now be asked multiple questions.")

    print("--------------------")
    print("How do you want to call your new device? (no space or special characters are allowed!)")
    device_name = input()
    print("You entered:", device_name)

    print("--------------------")
    print("Do you want to 'grab' the source device?")
    print("If you grab the source device, remapper will be the only application recieving events from this device!")
    grab_device = util.str2bool(input("grab?"))
    print("You entered:", grab_device)

    print("--------------------")
    print("Now, we're already at the last step!")
    print("You will be asked pairs of inputs. The first one is the input of the source device.")
    print("The second one, will be the mapped input.")
    print("You can enter the integer or the name of the code!")
    print("eg: BTN_Y or 308")

    event_map = dict()
    while True:
        source = input("source:")
        source = util.value_to_ecode(source)
        print("You entered:", source)

        dest = input("mapped:")
        dest = util.value_to_ecode(dest)
        print("You entered:", dest)
        event_map[source] = dest

        cont = util.str2bool(input("Continue? [y/n]"))
        if not cont:
            break

    print("--------------------")
    print("Do you want to change the bus type?")

    change_bustype = util.str2bool(input("Change?"))
    bustype = evdev.ecodes.BUS_USB
    if change_bustype:
        print("not implemented yet!")

    print("--------------------")
    print("Now you can change the vendor, product and version, which should be emulated.")
    print("There are also presets available:")

    idx = 0
    presets = util.uinput_presets()
    for preset in presets:
        print("[", idx, "]", preset['name'], preset['vendor'],
              preset['product'], preset['version'])
        idx = idx + 1

    print("If you want to apply a preset, enter the number of it.")
    print("To copy the values from source, enter -1")
    print("To manually enter values, enter -2")
    choice = int(input("Your choice?"))

    vendor = -1
    product = -1
    version = -1

    if choice == -1:
        vendor = device.vendor()
        product = device.product()
        version = device.version()
    elif choice == -2:
        vendor = int(input("vendor:"))
        product = int(input("product:"))
        version = int(input("version:"))
        pass
    elif choice > -1 and choice < len(presets):
        vendor = presets[choice]['vendor']
        product = presets[choice]['product']
        version = presets[choice]['version']
    else:
        print("Wrong pick! I'll copy it.")
        vendor = device.vendor()
        product = device.product()
        version = device.version()

    name_for_code = dict()
    for key, code in evdev.ecodes.ecodes.items():
        name_for_code[code] = key

    capabilities = {}
    for eventA, eventB in event_map.items():
        code_name = name_for_code[eventB]
        if 'BTN' in code_name or 'KEY' in code_name:
            if evdev.ecodes.EV_KEY not in capabilities:
                capabilities[evdev.ecodes.EV_KEY] = []

            capabilities[evdev.ecodes.EV_KEY].append(eventB)

        if 'ABS' in code_name:
            if evdev.ecodes.EV_KEY not in capabilities:
                capabilities[evdev.ecodes.EV_ABS] = []

            capabilities[evdev.ecodes.EV_ABS].append(eventB)

    output = outputdevice.OutputDevice(
        device_name, capabilities, bustype, vendor, product, version)
    remapper = remap.Remap(event_map, device, output, grab_device)
    CONFIG.add_remapper(remapper)


def edit():
    pass


def delete():
    pass


def run():
    remappers = CONFIG.get_remappers()
    for remapper in remappers:
        remapper.run()

def debug():
    device = util.select_evdev_via_console()
    for event in device.read_loop():
        print(event)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='remapper')
    parser.add_argument('--gui', help='run application in gui mode')
    parser.add_argument('--debug', help='check events of a evdev')
    args = parser.parse_args()

    if args.gui:
        app = QApplication(sys.argv)
        widget = gui.MyWidget()
        widget.resize(800, 600)
        widget.show()
        sys.exit(app.exec_())
    elif args.debug:
        debug()
    else:
        print("Welcome to remapper!")
        print("--------------------")
        print("This software allows you to remap a evdev device to your needs!")
        print("")
        
        main()
