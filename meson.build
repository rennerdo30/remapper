project('remapper', 'cpp')

evdev = dependency('libevdev', version : '1.9.0')
toml11 = declare_dependency(include_directories: 'lib/toml11-3.4.0/')
boost_dep = dependency('boost', modules : ['thread', 'program_options'])

project_source_files = [
  'src/main.cpp',
  'src/InputDevice.cpp',
  'src/KeyCodes.cpp',
  'src/InputMap.cpp',
  'src/OutputDevice.cpp'
]

executable('remapper', project_source_files, dependencies:[evdev, toml11, boost_dep])