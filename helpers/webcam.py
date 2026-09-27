"""Native OpenCV + MediaPipe face tracker; sends landmarks-derived scores locally.
No images leave this process. Run --help for usage. Camera starts only on launch.
"""
import argparse
import json
import math
import socket
import time
import cv2
import mediapipe as mp


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--model', required=True, help='Local face_landmarker.task path')
    p.add_argument('--camera', default='0', help='Camera index or IP camera stream URL')
    p.add_argument('--backend', choices=['auto', 'dshow', 'msmf', 'v4l2', 'avfoundation'], default='auto')
    p.add_argument('--width', type=int, default=640)
    p.add_argument('--height', type=int, default=480)
    p.add_argument('--fps', type=int, default=30)
    p.add_argument('--port', type=int, default=15483)
    p.add_argument('--preview', action='store_true')
    args = p.parse_args()
    options = mp.tasks.vision.FaceLandmarkerOptions(
        base_options=mp.tasks.BaseOptions(model_asset_path=args.model),
        running_mode=mp.tasks.vision.RunningMode.VIDEO,
        num_faces=1, output_face_blendshapes=True,
        output_facial_transformation_matrixes=True)
    backend = {'auto': cv2.CAP_ANY, 'dshow': cv2.CAP_DSHOW, 'msmf': cv2.CAP_MSMF,
               'v4l2': cv2.CAP_V4L2, 'avfoundation': cv2.CAP_AVFOUNDATION}[args.backend]
    source = int(args.camera) if args.camera.isdigit() else args.camera
    cap = cv2.VideoCapture(source, backend)
    if not cap.isOpened():
        raise RuntimeError('Cannot open webcam. Check camera permissions and index.')
    cap.set(cv2.CAP_PROP_FRAME_WIDTH, args.width)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, args.height)
    cap.set(cv2.CAP_PROP_FPS, args.fps)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    start = time.monotonic()
    previous = -1
    processed = 0
    try:
        with mp.tasks.vision.FaceLandmarker.create_from_options(options) as detector:
            while True:
                tick = time.monotonic()
                ok, frame = cap.read()
                if not ok:
                    raise RuntimeError(f'Camera frame acquisition failed after {processed} frames')
                timestamp = max(previous + 1, int((time.monotonic() - start) * 1000))
                previous = timestamp
                image = mp.Image(image_format=mp.ImageFormat.SRGB,
                                 data=cv2.cvtColor(frame, cv2.COLOR_BGR2RGB))
                result = detector.detect_for_video(image, timestamp)
                processed += 1
                if result.face_blendshapes:
                    scores = {v.category_name: float(v.score) for v in result.face_blendshapes[0]}
                    if result.facial_transformation_matrixes:
                        m = result.facial_transformation_matrixes[0]
                        # Native MediaPipe matrix is row-major. Euler angles in degrees.
                        head = {'pitch': math.degrees(math.atan2(m[2, 1], m[2, 2])),
                                'yaw': math.degrees(math.atan2(-m[2, 0], math.hypot(m[0, 0], m[1, 0]))),
                                'roll': math.degrees(math.atan2(m[1, 0], m[0, 0]))}
                    else:
                        head = {}
                    payload = {'source': 'webcam', 'blendshapes': scores, 'head': head}
                    sock.sendto(json.dumps(payload).encode(), ('127.0.0.1', args.port))
                if args.preview:
                    cv2.imshow('Valkyrie Studio webcam - Esc to quit', frame)
                    if cv2.waitKey(1) & 0xff == 27:
                        break
                time.sleep(max(0, 1 / max(1, args.fps) - (time.monotonic() - tick)))
    except KeyboardInterrupt:
        pass
    finally:
        cap.release()
        sock.close()
        cv2.destroyAllWindows()


if __name__ == '__main__':
    main()
