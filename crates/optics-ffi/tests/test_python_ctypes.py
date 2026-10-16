#!/usr/bin/env python3
"""
Integration test for OpticsLab C FFI ABI using Python's native ctypes standard library.
Demonstrates zero-dependency Python interoperability with the Rust computational backend.
"""

import ctypes
import json
import os
import platform
import sys

def get_dylib_path():
    root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../../"))
    system = platform.system()
    if system == "Darwin":
        filename = "liboptics_ffi.dylib"
    elif system == "Windows":
        filename = "optics_ffi.dll"
    else:
        filename = "liboptics_ffi.so"
    
    path = os.path.join(root, "target", "debug", filename)
    if not os.path.exists(path):
        raise FileNotFoundError(f"Dynamic library not found at: {path}")
    return path

def test_ctypes_bindings():
    dylib_path = get_dylib_path()
    lib = ctypes.CDLL(dylib_path)

    # Function signatures
    lib.optics_engine_create.restype = ctypes.c_void_p
    lib.optics_engine_create.argtypes = []

    lib.optics_engine_free.restype = None
    lib.optics_engine_free.argtypes = [ctypes.c_void_p]

    lib.optics_string_free.restype = None
    lib.optics_string_free.argtypes = [ctypes.c_char_p]

    lib.optics_engine_get_status_json.restype = ctypes.c_void_p
    lib.optics_engine_get_status_json.argtypes = [ctypes.c_void_p]

    lib.optics_engine_run_analysis_json.restype = ctypes.c_void_p
    lib.optics_engine_run_analysis_json.argtypes = [ctypes.c_void_p, ctypes.c_char_p]

    lib.optics_engine_execute_command_json.restype = ctypes.c_void_p
    lib.optics_engine_execute_command_json.argtypes = [ctypes.c_void_p, ctypes.c_char_p]

    lib.optics_engine_export_svg.restype = ctypes.c_void_p
    lib.optics_engine_export_svg.argtypes = [ctypes.c_void_p]

    # 1. Create Engine
    engine = lib.optics_engine_create()
    assert engine is not None, "Failed to create OpticsEngine via C ABI"

    try:
        # 2. Get Status JSON
        status_ptr = lib.optics_engine_get_status_json(engine)
        assert status_ptr is not None
        status_str = ctypes.cast(status_ptr, ctypes.c_char_p).value.decode("utf-8")
        lib.optics_string_free(ctypes.cast(status_ptr, ctypes.c_char_p))
        status = json.loads(status_str)
        print(f"[Python ctypes] Loaded system: {status['system_name']}, surfaces: {status['surface_count']}, EFL: {status['efl_mm']:.2f} mm")
        assert status["system_name"] == "Cooke Triplet F/4.5"
        assert abs(status["efl_mm"] - 50.0) < 0.1

        # 3. Run Paraxial Analysis
        req_json = json.dumps({"type": "Paraxial"}).encode("utf-8")
        analysis_ptr = lib.optics_engine_run_analysis_json(engine, req_json)
        assert analysis_ptr is not None
        analysis_str = ctypes.cast(analysis_ptr, ctypes.c_char_p).value.decode("utf-8")
        lib.optics_string_free(ctypes.cast(analysis_ptr, ctypes.c_char_p))
        analysis = json.loads(analysis_str)
        assert analysis["result_type"] == "Paraxial"
        print(f"[Python ctypes] Paraxial analysis EFL: {analysis['data']['efl_mm']:.4f} mm, F/#: {analysis['data']['f_number']:.2f}")

        # 4. Run Spot Diagram Analysis
        spot_req = json.dumps({"type": "SpotDiagram", "params": {"field_index": 0, "ray_count": 64}}).encode("utf-8")
        spot_ptr = lib.optics_engine_run_analysis_json(engine, spot_req)
        assert spot_ptr is not None
        spot_str = ctypes.cast(spot_ptr, ctypes.c_char_p).value.decode("utf-8")
        lib.optics_string_free(ctypes.cast(spot_ptr, ctypes.c_char_p))
        spot = json.loads(spot_str)
        rms_um = spot["data"]["metrics"]["rms_radius_um"]
        print(f"[Python ctypes] Spot diagram RMS blur radius: {rms_um:.2f} µm")
        assert rms_um < 50.0

        # 5. Execute Command: Modify EPD
        cmd_json = json.dumps({"command": "SetEntrancePupilDiameter", "args": {"epd_mm": 12.5}}).encode("utf-8")
        cmd_ptr = lib.optics_engine_execute_command_json(engine, cmd_json)
        assert cmd_ptr is not None
        cmd_str = ctypes.cast(cmd_ptr, ctypes.c_char_p).value.decode("utf-8")
        lib.optics_string_free(ctypes.cast(cmd_ptr, ctypes.c_char_p))
        cmd_res = json.loads(cmd_str)
        assert cmd_res["success"] is True
        print(f"[Python ctypes] Command executed successfully: revision {cmd_res['revision']}")

        # 6. Export SVG Layout
        svg_ptr = lib.optics_engine_export_svg(engine)
        assert svg_ptr is not None
        svg_str = ctypes.cast(svg_ptr, ctypes.c_char_p).value.decode("utf-8")
        lib.optics_string_free(ctypes.cast(svg_ptr, ctypes.c_char_p))
        assert "<svg" in svg_str
        print(f"[Python ctypes] Exported SVG layout ({len(svg_str)} characters)")

        print("[Python ctypes] ALL FFI CHECKS PASSED SUCCESSFULLY!")

    finally:
        lib.optics_engine_free(engine)

if __name__ == "__main__":
    test_ctypes_bindings()
