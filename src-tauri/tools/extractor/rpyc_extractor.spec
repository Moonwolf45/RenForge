# -*- mode: python ; coding: utf-8 -*-

import os

# Вендоренный unrpyc (для режима --decompile «экспертного просмотра»).
# Копируем .py-дерево (unrpyc.py, deobfuscate.py, пакет decompiler/) в бандл под unrpyc/,
# исключая тяжёлые/ненужные каталоги. main.py добавит _MEIPASS/unrpyc в sys.path.
_UNRPYC = r'd:\Renforge\Renforge\renforge\src-tauri\tools\unrpyc'
_SKIP = {'build', 'dist', '.github', '__pycache__', 'testcases', 'un.rpyc', '.git'}
_unrpyc_datas = []
for _root, _dirs, _files in os.walk(_UNRPYC):
    _dirs[:] = [d for d in _dirs if d not in _SKIP]
    for _fn in _files:
        if _fn.endswith('.py'):
            _full = os.path.join(_root, _fn)
            _rel = os.path.relpath(_root, _UNRPYC)
            _dest = 'unrpyc' if _rel == '.' else os.path.join('unrpyc', _rel)
            _unrpyc_datas.append((_full, _dest))

a = Analysis(
    ['d:\\Renforge\\Renforge\\renforge\\src-tauri\\tools\\extractor\\main.py'],
    pathex=[],
    binaries=[],
    datas=_unrpyc_datas,
    hiddenimports=['pickletools', 'multiprocessing', 'multiprocessing.pool', 'multiprocessing.synchronize'],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='rpyc_extractor',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    version=r'd:\Renforge\Renforge\renforge\src-tauri\tools\extractor\version_info.txt',
)
