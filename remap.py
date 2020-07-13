import threading
import evdev
import asyncio

import outputdevice
import inputdevice
import util


def do_work(loop, in_dev, out_dev, event_map):
    asyncio.set_event_loop(loop)
    for event in in_dev.read_loop():
        new_event_code = event.code
        if new_event_code in event_map:
            new_event_code = event_map[new_event_code]
        
        value = event.value
        if event.type == evdev.ecodes.EV_ABS:
            value = int(util.translate(value, 0, 255, -32768, 32767))
        
        out_dev.write(event.type, new_event_code, event.value)
        out_dev.syn()


class Remap:
    def __init__(self, event_map, inputdevice, outputdevice, grab_device=True):
        self.event_map = event_map
        self.inputdevice = inputdevice
        self.outputdevice = outputdevice
        self.grab_device = grab_device

    def run(self):
        print("Starting", "'" + self.inputdevice.name() + "'",
              "to", "'" + self.outputdevice.name + "'", "mapping")

        if self.grab_device:
            self.inputdevice.grab()

        loop = asyncio.new_event_loop()
        x = threading.Thread(target=do_work, args=(loop,
                                                   self.inputdevice, self.outputdevice, self.event_map))
        x.start()

    def stop(self):
        if self.grab_device:
            self.inputdevice.ungrab()

    def to_dict(self):
        data = dict()
        data['event_map'] = self.event_map
        data['grab_device'] = self.grab_device
        data['inputdevice'] = self.inputdevice.to_dict()
        data['outputdevice'] = self.outputdevice.to_dict()
        return data
