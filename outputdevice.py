import evdev


def from_json(json):
    capabilities = {}
    for key, value in json['capabilities'].items():
        capabilities[int(key)] = value

    return OutputDevice(json['name'], capabilities, json['bustype'], json['vendor'], json['product'], json['version'])


class OutputDevice:
    def __init__(self, name, capabilities, bustype, vendor, product, version):
        self.name = name
        self.capabilities = capabilities
        self.bustype = bustype
        self.vendor = vendor
        self.product = product
        self.version = version

        self.device = evdev.uinput.UInput(name=self.name, events=self.capabilities,
                                          bustype=self.bustype, vendor=self.vendor,
                                          product=self.product, version=self.version)

    def to_dict(self):
        data = dict()
        data['name'] = self.name
        data['capabilities'] = self.capabilities
        data['bustype'] = self.bustype
        data['vendor'] = self.vendor
        data['product'] = self.product
        data['version'] = self.version
        return data
