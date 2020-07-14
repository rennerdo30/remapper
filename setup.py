from setuptools import setup, find_packages
setup(
    name="remapper",
    version="1.0.0",
    packages=find_packages(),
    scripts=["config.py", "gui.py", "inputdevice.py", "outputdevice.py", "remapper.py", "remap.py", "setup.py", "util.py", "add_remap_ui.py", "remapper_ui.py"],
    install_requires=["PyQt5_sip>=12.8.0", "evdev>=1.3.0", "PySide2>=5.15.0"],
)