"""File GPU dependencies joined by output routes, producer tokens and renderer uploads."""
from bisect import bisect_right
from collections import Counter, defaultdict
import json
from pathlib import Path
import runpy

GPU_PATH = runpy.run_path(str(Path(__file__).with_name('production_gpu_path.py')))


def key(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def unique(rows, identity):
    result = {}
    for row in rows:
        value = identity(row)
        if value in result:
            raise ValueError('duplicate Video GPU-path identity')
        result[value] = row
    return result


def upload_owner(row):
    return key(row['token']['device']), row['texture'], row['stream']


def direct_upload(use, frame, uses, renderer_frames, uploads):
    # ImageStream sequences are immutable within a lifetime. The latest write to
    # this device/texture/stream must already exist when the retained pixels are read.
    if use.get('software_upload') is not None or not isinstance(use.get('frame'), dict):
        return None
    writes = uploads.get(upload_owner(use), ())
    index = bisect_right(writes, frame['token']['serial'], key=lambda row: row['token']['serial'])
    if not index:
        return None
    upload = writes[index - 1]
    if upload['sequence'] != use['sequence']:
        return None
    origin = key(upload['token'])
    original = uses.get((origin, use['texture']))
    submitted = renderer_frames.get(origin)
    descriptor = use.get('descriptor')
    if (not original or not submitted or not isinstance(descriptor, dict)
            or original.get('descriptor') != descriptor
            or original.get('stream') != use['stream'] or original.get('sequence') != use['sequence']
            or original.get('frame') != use['frame']
            or original.get('publication') is not None or original.get('software_upload') is not None):
        return None
    extent = descriptor.get('extent')
    if not isinstance(extent, dict):
        return None
    width, height = extent.get('width'), extent.get('height')
    if (descriptor.get('format') not in ('Rgba8Unorm', 'Bgra8Unorm')
            or type(width) is not int or width <= 0 or type(height) is not int or height <= 0):
        return None
    payload = upload.get('row_pitch'), upload.get('rows'), upload.get('bytes')
    if payload != (width * 4, height, width * height * 4):
        return None
    intervals = [submitted.get('wall'), submitted.get('inputs'), upload.get('cpu'), frame.get('wall')]
    if any(not isinstance(value, dict) for value in intervals):
        return None
    wall, inputs, api, drawing = intervals
    clocks = [wall.get('begin'), inputs.get('begin'), api.get('begin'), api.get('end'),
              inputs.get('end'), wall.get('end'), drawing.get('begin')]
    if any(type(value) is not int or not 0 < value < 1 << 64 for value in clocks):
        return None
    if clocks[:6] != sorted(clocks[:6]) or (origin != key(frame['token']) and clocks[3] > clocks[6]):
        return None
    return {'texture': use['texture'], 'token': upload['token'], 'begin_qpc': api['begin']}


def analyze_video_gpu_path(frames, native, bindings, frequency, distribution, *, healthy):
    routes, outputs = defaultdict(list), defaultdict(list)
    unique(frames, lambda row: key(row['token']))
    unique(native['OutputSource'], lambda row: (key(row['generation']), row['output'], row['texture']))
    unique(native['Output'], lambda row: (key(row['token']), row['output']))
    for row in native['OutputSource']:
        routes[(key(row['generation']), row['output'])].append(row)
    for row in native['Output']:
        outputs[key(row['token'])].append(row['output'])
    submissions = unique(native['SourceSubmission'], lambda row: key(row['token']))
    renderer_frames = unique(native.get('FrameCpu', frames), lambda row: key(row['token']))
    uploads = defaultdict(list)
    for upload in unique(native.get('ImageUpload', []), lambda row: (key(row['token']), row['texture'])).values():
        uploads[upload_owner(upload)].append(upload)
    for writes in uploads.values():
        writes.sort(key=lambda row: row['token']['serial'])
    streams = {row['stream'] for row in bindings}
    expected, selected, source_free = {}, [], 0
    for frame in frames:
        identity = key(frame['token'])
        missing, textures = [], set()
        drawn = outputs[identity]
        if not drawn:
            missing.append('renderer output observations missing')
        for output in drawn:
            facts = routes[(key(frame['generation']), output)]
            counts = {row['source_count'] for row in facts}
            actual = {row['texture'] for row in facts if row['texture'] is not None}
            valid = (len(counts) == 1 and all(type(value) is int and value >= 0 for value in counts)
                     and next(iter(counts)) == len(actual)
                     and all(type(value) is int and value > 0 for value in actual)
                     and (all(row['texture'] is not None for row in facts) if actual
                          else len(facts) == 1 and facts[0]['texture'] is None))
            if not valid:
                missing.append('missing/incomplete actual output source table')
            textures.update(actual)
        if not textures and not missing:
            source_free += 1
            continue
        expected[identity] = textures, missing
        selected.append(frame)

    def dependencies(frame, uses, sources):
        identity = key(frame['token'])
        textures, reasons = expected[identity]
        missing, result, inline, seen = list(reasons), [], [], set()
        for texture in sorted(textures):
            use = uses.get((identity, texture))
            stream = use.get('stream') if use else None
            if not use or stream not in streams or type(use.get('sequence')) is not int or use['sequence'] <= 0:
                missing.append(f'exact File producer dependency missing/foreign for texture {texture}')
                continue
            publication = use.get('publication')
            if publication is None:
                upload = direct_upload(use, frame, uses, renderer_frames, uploads)
                if upload:
                    inline.append(upload)
                else:
                    missing.append(f'exact renderer File upload missing/foreign for texture {texture}')
                continue
            origin = key(publication.get('source_token'))
            source, submission = sources.get(origin), submissions.get(origin)
            if (not source or not submission
                    or source.get('context') != {'FrameProducer': {'stream': stream}}
                    or submission.get('stream') != stream
                    or not isinstance(use.get('descriptor'), dict)
                    or submission.get('descriptor') != use['descriptor']):
                missing.append(f'exact File producer dependency missing/foreign for texture {texture}')
                continue
            # Aliases/References retain one actual submission, not one charge per texture.
            if origin not in seen:
                result.append((source, source['context']))
                seen.add(origin)
        return result, inline, missing

    result = GPU_PATH['analyze_gpu_path'](selected, native, frequency, distribution,
                                         healthy=healthy, resolve_dependencies=dependencies,
                                         include_context_gaps=False)
    reasons = Counter(reason for row in result['records'] for reason in row['unknown_reasons'])
    result.update(input_renderer_batches=len(frames), excluded_proven_source_free_batches=source_free,
                  missing_reasons=dict(reasons),
                  scope='Actual File-dependent output batches, exact generation/texture routes, retained producer '
                        'tokens and renderer uploads; the current renderer span includes other rendering/output work. Only proven '
                        'source-free batches are excluded and counted separately.')
    result['limitations'].append('An exact held same-context upload extends the earliest CPU bound. '
                                 'It never charges a previous batch’s unrelated GPU work. Missing upload rows and foreign '
                                 'or unbound sources remain incomplete.')
    result['limitations'].append('Intentional File-frame retention contributes only to the conservative upper '
                                 'enclosure. Failure uses the largest quantization-corrected local GPU span; '
                                 'retention gaps and serial span sums cannot prove slow GPU execution.')
    return result
