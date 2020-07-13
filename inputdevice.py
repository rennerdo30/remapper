import evdev


def from_json(json):
    devices = [evdev.InputDevice(path) for path in evdev.list_devices()]
    for device in devices:
        if device.name == json['name'] and device.phys == json['phys'] and device.version == json['version'] and device.info.vendor == json['vendor'] and device.info.product == json['product']:
            return InputDevice(device)

    return None


class InputDevice:
    def __init__(self, device):
        self.device = device

    def name(self):
        return self.device.name

    def info_string(self):
        return self.device.name + " " + self.device.phys + " [" + self.device.path + "]"

    def capabilities(self):
        return self.device.capabilities(absinfo=False)

    def vendor(self):
        return self.device.vendor

    def version(self):
        return self.device.version

    def product(self):
        return self.device.product

    def grab(self):
        return self.device.grab()

    def ungrab(self):
        return self.device.ungrab()

    def read_loop(self):
        return self.device.read_loop()

    def to_dict(self):
        data = dict()
        data['name'] = self.device.name
        data['phys'] = self.device.phys
        data['version'] = self.device.version
        data['vendor'] = self.device.info.vendor
        data['product'] = self.device.info.product
        return data
