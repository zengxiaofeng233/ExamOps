"""Local inference selection and bounded CPU parallelism (no telemetry)."""
import os
import sys

GIB = 1024 ** 3


def available_worker_slots(available=None, total=None):
    import psutil
    memory = psutil.virtual_memory()
    available = memory.available if available is None else available
    total = memory.total if total is None else total
    reserve = max(2 * GIB, total // 4)
    # Each worker owns its models and render buffers. Leave room for the app/OS.
    return max(0, int((available - reserve) // (1536 * 1024 ** 2)))


def worker_limit(available=None, total=None, cpus=None):
    return max(1, min(4, max(1, (cpus or os.cpu_count() or 2) // 2),
                      available_worker_slots(available, total)))


def provider_for(mode):
    import onnxruntime as ort
    if mode != "cpu" and sys.platform == "win32" and "DmlExecutionProvider" in ort.get_available_providers():
        return "directml"
    return "cpu"


def preferred_mode(mode):
    """Plan workers without importing an inference engine for native-text PDFs."""
    import importlib.metadata
    if mode != "cpu" and sys.platform == "win32":
        try:
            importlib.metadata.version("onnxruntime-directml")
            return "directml"
        except importlib.metadata.PackageNotFoundError:
            pass
    return "cpu"


def make_engine(mode):
    import rapidocr
    import onnxruntime as ort
    from pathlib import Path
    from rapidocr import RapidOCR
    models = Path(rapidocr.__file__).parent / "models"
    paths = {"Det": "PP-OCRv6_det_small.onnx", "Cls": "ch_ppocr_mobile_v2.0_cls_mobile.onnx",
             "Rec": "PP-OCRv6_rec_small.onnx"}
    params = {"Global.log_level": "error", "Global.max_side_len": 3000,
              "EngineConfig.onnxruntime.intra_op_num_threads": 2,
              "EngineConfig.onnxruntime.inter_op_num_threads": 1}
    params.update({f"{stage}.model_path": str(models / name) for stage, name in paths.items()})
    engine = RapidOCR(params=params)
    if mode == "directml":
        # DirectML requires sequential execution and disabled memory patterns.
        # Inject sessions into the config after OmegaConf has parsed primitive params.
        for stage, name in paths.items():
            options = ort.SessionOptions()
            options.enable_mem_pattern = False
            options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            options.intra_op_num_threads = 2
            options.log_severity_level = 4
            session = ort.InferenceSession(str(models / name), sess_options=options,
                providers=[("DmlExecutionProvider", {"device_id": 0}), "CPUExecutionProvider"])
            if session.get_providers()[0] != "DmlExecutionProvider":
                raise RuntimeError("DirectML is unavailable")
            engine.cfg[stage]._set_flag("allow_objects", True)
            engine.cfg[stage]["session"] = session
    return engine


def watch_parent(parent_pid):
    """Also stop frozen workers after a forced app termination on Windows."""
    import psutil
    import threading
    import time
    try:
        parent = psutil.Process(parent_pid)
    except psutil.NoSuchProcess:
        os._exit(0)
    def monitor():
        while parent.is_running():
            time.sleep(1)
        os._exit(0)
    threading.Thread(target=monitor, daemon=True).start()
