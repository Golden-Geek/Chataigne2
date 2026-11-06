"""M12/10: decode actual SDK captures against guarded CPU glyph references.

References are emitted by the checked app font-raster test. Geometry, placement, linear
coverage, time bounds and palette expectations are evaluated independently of the shader.
"""
import argparse
from datetime import datetime, timedelta
import json
import math
from pathlib import Path
import re
from PIL import Image
from calibration_pixels import homography, srgb


def unorm_interval(minimum, maximum=None):
    """D3D11.3 3.2.3.6 permits 0.6 integer ULP per FLOAT->UNORM conversion.

    Propagate those discrete storage choices at each actual 8-bit target, rather than
    assuming all adapters implement ideal Python round-to-even at every intermediate.
    https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm
    """
    maximum = minimum if maximum is None else maximum
    return (max(0, math.ceil(minimum*255-.6))/255,
            min(255, math.floor(maximum*255+.6))/255)


def inverse(m):
    a, b, c, d, e, f, g, h, i = m
    cofactors = [e*i-f*h, c*h-b*i, b*f-c*e, f*g-d*i, a*i-c*g, c*d-a*f,
                 d*h-e*g, b*g-a*h, a*e-b*d]
    determinant = a*cofactors[0] + b*cofactors[3] + c*cofactors[6]
    return [v/determinant for v in cofactors]


def project(m, x, y):
    z = m[6]*x + m[7]*y + m[8]
    return (m[0]*x+m[1]*y+m[2])/z, (m[3]*x+m[4]*y+m[5])/z


class Font:
    def __init__(self, directory, size):
        self.image = Image.open(directory / f"font-{size}.png").getchannel("A")
        self.width, self.height = self.image.width//16, self.image.height//6

    def coverage(self, char, x, y):
        slot = ord(char)-32
        x += slot % 16*self.width-0.5
        y += slot // 16*self.height-0.5
        ix, iy = math.floor(x), math.floor(y)
        wx, wy = x-ix, y-iy
        def at(dx, dy):
            return self.image.getpixel((max(0, min(self.image.width-1, ix+dx)),
                                        max(0, min(self.image.height-1, iy+dy))))/255
        return ((1-wx)*at(0, 0)+wx*at(1, 0))*(1-wy) + ((1-wx)*at(0, 1)+wx*at(1, 1))*wy


def inspect(directory, observation, references):
    label = observation["label"]
    image = Image.open(directory/label/"pixels.png").convert("RGBA")
    width, height = image.size
    assert [width, height] == [observation["pixels"]["width"], observation["pixels"]["height"]]
    if label == "restored-source":
        assert image.getextrema() == ((255,255), (0,0), (0,0), (255,255))
        return {"label": label, "every_pixel_red": True}
    diameter = min(width, height)
    cell = max(diameter/9, max(width, height)/32, 1)
    font = Font(references, min(192, max(8, math.ceil(diameter*.065))))
    glyph_height = min(font.height*.5, diameter*.06)
    advance = glyph_height*font.width/font.height
    matrix = homography() if label == "warped" else [1,0,0,0,1,0,0,0,1]
    undo = inverse(matrix)
    smooth = observation.get("anti_aliasing") == "Smooth2x"

    def observed(x, y):
        px, py = project(matrix, x/width, y/height)
        px, py = math.floor(px*width), math.floor(py*height)
        assert 0 <= px < width and 0 <= py < height
        sx, sy = project(undo, (px+.5)/width, (py+.5)/height)
        return image.getpixel((px, py)), sx*width, sy*height

    def line(length, top, alphabet):
        start = (width-length*advance)/2
        result = ""
        mismatches = checked = 0
        for index in range(length):
            points = []
            for row in range(24):
                for column in range(16):
                    pixel, sx, sy = observed(start+(index+(column+.5)/16)*advance,
                                             top+(row+.5)/24*glyph_height)
                    local_x = (sx-start-index*advance)/advance*font.width
                    local_y = (sy-top)/glyph_height*font.height
                    if not (0 <= local_x < font.width and 0 <= local_y < font.height):
                        continue
                    points.append((min(pixel[:3]) >= 220, local_x, local_y))
            choices = []
            for char in alphabet:
                errors = count = ink = 0
                for actual, x, y in points:
                    alpha = font.coverage(char, x, y)
                    if alpha < .05 or alpha > .95:
                        errors += actual != (alpha > .95)
                        ink += alpha > .95
                        count += 1
                choices.append((errors/max(1,count), errors, count, char, ink))
            fraction, errors, count, char, ink = min(choices)
            assert count >= 100, (label, "insufficient glyph coverage", count)
            result += char
            mismatches += errors
            checked += count
        return result, mismatches/max(1,checked), checked

    def pixel_oracle(text, top):
        start = (width-len(text)*advance)/2
        points = set()
        for index in range(len(text)):
            for row in range(24):
                for column in range(16):
                    px, py = project(matrix, (start+(index+(column+.5)/16)*advance)/width,
                                     (top+(row+.5)/24*glyph_height)/height)
                    points.add((math.floor(px*width), math.floor(py*height)))

        def sample(x, y):
            sx, sy = project(undo, x/width, y/height)
            sx, sy = sx*width, sy*height
            if not (start <= sx < start+len(text)*advance and top <= sy < top+glyph_height):
                return None
            # Exclude other painted primitives; retain every coverage value, including
            # antialiased edges, at the remaining independently located sample pixels.
            cx, cy = sx % cell, sy % cell
            if min(cx, cell-cx, cy, cell-cy) <= 3:
                return None
            if cell*.06-1 <= cy <= cell*.18+1:
                return None
            diagonal = math.sqrt(1+(height/width)**2)
            if min(abs(sy-sx*height/width), abs(sy-height+sx*height/width))/diagonal <= 3:
                return None
            if abs(math.hypot(sx-width/2, sy-height/2)-diameter/2) <= 3:
                return None
            index = min(len(text)-1, math.floor((sx-start)/advance))
            alpha = font.coverage(text[index], ((sx-start)/advance-index)*font.width,
                                  (sy-top)/glyph_height*font.height)
            grey = .1 if (math.floor(sx/cell)+math.floor(sy/cell)) % 2 == 0 else .2
            return round((grey*(1-alpha)+.95*alpha)*255)/255

        checked = maximum = edge_points = 0
        offsets = [.25,.75] if smooth else [.5]
        for px, py in sorted(points):
            values = [sample(px+dx, py+dy) for dx in offsets for dy in offsets]
            if any(value is None for value in values):
                continue
            linear = round(sum(values)/len(values)*255)/255
            expected = srgb(linear)
            actual = image.getpixel((px, py))
            error = max(abs(value-expected) for value in actual[:3])
            assert actual[3] == 255 and error <= 2, (label, text, (px,py), actual, expected, error)
            checked += 1
            maximum = max(maximum,error)
            edge_points += srgb(.2)+8 < expected < srgb(.95)-8
        assert checked >= 500 and edge_points >= 10, (label, text, checked, edge_points)
        return {"pixels":checked, "edge_pixels":edge_points, "maximum_rgb_error":maximum}

    def cell_label_oracle():
        minor = Font(references, min(64, max(4, math.ceil(cell*.18))))
        label_height = min(cell*.12, diameter*.05)
        label_advance = label_height*minor.width/minor.height
        left, top = cell*1.06, cell*3.06
        checked = maximum = fractional = nominal_maximum = 0
        offsets = [.25,.75] if smooth else [.5]
        points = set()
        for index in range(2):
            for row in range(24):
                for column in range(16):
                    x,y = project(matrix, (left+(index+(column+.5)/16)*label_advance)/width,
                                  (top+(row+.5)/24*label_height)/height)
                    points.add((math.floor(x*width), math.floor(y*height)))
        for px,py in points:
            values = []
            for dx in offsets:
                for dy in offsets:
                    x,y = project(undo, (px+dx)/width, (py+dy)/height)
                    x,y = x*width,y*height
                    char = math.floor((x-left)/label_advance)
                    if not (0 <= char < 2 and top <= y < top+label_height):
                        values.append(None)
                        continue
                    alpha = minor.coverage("B4"[char], ((x-left)/label_advance-char)*minor.width,
                                           (y-top)/label_height*minor.height)
                    values.append(.5*.55*alpha+.1*(1-.55*alpha))
            if any(v is None for v in values):
                continue
            stored = [unorm_interval(value) for value in values]
            lower, upper = (sum(v[i] for v in stored)/len(stored) for i in [0,1])
            if smooth:
                lower, upper = unorm_interval(lower, upper)
            low, high = srgb(lower), srgb(upper)
            expected = srgb(round(sum(round(v*255)/255 for v in values)/len(values)*255)/255)
            actual = image.getpixel((px,py))
            error = max(max(low-value,value-high,0) for value in actual[:3])
            nominal_maximum = max(nominal_maximum, max(abs(value-expected) for value in actual[:3]))
            assert actual[3] == 255 and error <= 2, (label, "B4", (px,py), actual, (low,high), error)
            checked += 1
            fractional += srgb(.1)+8 < expected < srgb(.32)-8
            maximum = max(maximum,error)
        assert checked >= 50 and fractional >= 5, (label, checked, fractional)
        assert label_height < glyph_height*.5
        return {"label":"B4", "pixels":checked, "fractional_pixels":fractional,
                "maximum_rgb_error_outside_d3d_storage_interval":maximum,
                "maximum_rgb_error_against_ideal_rounding":nominal_maximum, "height":label_height}

    top = (height-diameter*.24)/2-glyph_height-diameter*.02
    clock, error, points = line(12, top, "0123456789:.")
    assert error <= .01 and re.fullmatch(r"\d\d:\d\d:\d\d\.\d\d\d", clock), (label, clock, error)
    before, after = observation["before"]/1000, observation["after"]/1000
    stamp = datetime.fromtimestamp(before).replace(hour=int(clock[:2]), minute=int(clock[3:5]),
        second=int(clock[6:8]), microsecond=int(clock[9:])*1000)
    assert any(before-.5 <= (stamp+timedelta(days=d)).timestamp() <= after+.1 for d in [-1,0,1]), (label, clock)
    below = (height+diameter*.24)/2+diameter*.025
    expected = f"{width} x {height}"
    resolution, resolution_error, count = line(len(expected), below, "0123456789 x")
    points += count
    assert resolution == expected and resolution_error <= .01, (label, resolution, expected, resolution_error)
    pixel_checks = [pixel_oracle(clock, top), pixel_oracle(resolution, below)]
    rates = []
    for length in range(7,12):
        value, error, count = line(length, below+glyph_height+diameter*.008, "0123456789. FPS-")
        if error <= .01 and re.fullmatch(r"(?:\d+\.\d|--\.-) FPS", value):
            rates.append(value)
            points += count
    assert len(rates) == 1, (label, "FPS candidates", rates)
    fps = None if rates[0].startswith("--") else float(rates[0].split()[0])
    pixel_checks.append(pixel_oracle(rates[0], below+glyph_height+diameter*.008))
    if fps is not None:
        assert fps > 0 and abs(fps-observation["runtime"]["submitted_fps"]) < 5, (label, fps, observation["runtime"])
    for index in range(8):
        actual, _, _ = observed((index+.4)*width/8, cell*1.5)
        assert all(abs(v-srgb(index/7)) <= 2 for v in actual[:3]), (label, index, actual)
        expected = [(255,255,255),(255,255,0),(0,255,255),(0,255,0),
                    (255,0,255),(255,0,0),(0,0,255),(0,0,0)][index]
        actual, _, _ = observed((index+.4)*width/8, height-cell*1.5)
        assert all(abs(a-b) <= 2 for a,b in zip(actual[:3], expected)), (label, index, actual)
    return {"label":label, "clock":clock, "resolution":resolution, "fps":fps,
            "glyph_classification_points":points, "readout_cell_height":glyph_height,
            "font_raster_cell_height":font.height, "palette_points":16, "warp":label == "warped",
            "linear_coverage_pixels":pixel_checks, "subordinate_cell_label":cell_label_oracle()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--glyphs", required=True, type=Path)
    args = parser.parse_args()
    result = json.loads((args.directory/"results.json").read_text())
    checks = [inspect(args.directory, {**row, "anti_aliasing":result["anti_aliasing"]}, args.glyphs)
              for row in result["observations"]]
    live = {r["label"]:r for r in checks if r["label"].startswith("live-")}
    assert live["live-a"]["clock"] != live["live-b"]["clock"]
    assert live["live-b"]["fps"] is not None
    assert result["errors"] == [] and result["retirementCountAfter"] > result["retirementCountBefore"]
    report = {"passed":True, "gpu":result["gpu"], "anti_aliasing":result["anti_aliasing"],
              "checks":checks, "scope":"ASCII identity (interior classification tolerance 1%), live time/FPS, native size/palette, sampled linear glyph coverage including edges at unchanged RGB tolerance 2; not whole-image equality"}
    (args.directory/"typography-results.json").write_text(json.dumps(report, indent=2)+"\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
