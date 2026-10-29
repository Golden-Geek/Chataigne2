"""Join PresentMon v1 display observations to exact production window API calls."""

from bisect import bisect_right
from collections import defaultdict
from fractions import Fraction
import json
from pathlib import Path
import runpy

DATA = runpy.run_path(str(Path(__file__).with_name('presentmon_data.py')))


def _key(value):
    return json.dumps(value, sort_keys=True)


def _unavailable(reason):
    return {'captured': False, 'coverage_complete': False, 'reason': reason, 'outputs': []}


def analyze_presentmon(report, native, sources, window, pid, frequency, distribution):
    if report is None:
        return _unavailable('optional PID-filtered PresentMon capture is absent')
    if not report.get('stdout'):
        return _unavailable(report.get('error', 'collector returned no CSV'))
    if report.get('target_pid') != pid:
        raise ValueError('PresentMon target PID differs from the native renderer')
    _, groups, total = DATA['_read'](report['stdout'], pid, frequency)
    calls, aliases, starts = [], defaultdict(list), {}
    for output in native['Output']:
        identity = output.get('window_present')
        api = output.get('consumer_api')
        if identity is None:
            continue
        if output['output'] not in (1, 3) or api is None:
            raise ValueError('window identity is not an actual native window API call')
        if type(identity['generation']) is not int or identity['generation'] < 1:
            raise ValueError('invalid window endpoint generation')
        addresses = identity['addresses']
        if not isinstance(addresses, list) or len(addresses) != 3 or any(
                type(a) is not int or not 0 < a < (1 << 64) for a in addresses):
            raise ValueError('window identity requires all three native interface aliases')
        if type(api['begin']) is not int or type(api['end']) is not int or not 0 < api['begin'] <= api['end']:
            raise ValueError('invalid native window API interval')
        entry = {'output': output, 'csv': None}
        calls.append(entry)
        for address in set(addresses):
            aliases[hex(address)].append(entry)
    for address, entries in aliases.items():
        entries.sort(key=lambda e: e['output']['consumer_api']['begin'])
        for left, right in zip(entries, entries[1:]):
            if left['output']['consumer_api']['end'] >= right['output']['consumer_api']['begin']:
                raise ValueError('overlapping window API aliases are ambiguous')
        starts[address] = [e['output']['consumer_api']['begin'] for e in entries]
    unmatched, rejected = [], []
    for address, rows in groups.items():
        entries = aliases.get(address, [])
        for row in rows:
            index = bisect_right(starts.get(address, []), row.qpc)-1
            if index < 0 or row.qpc > entries[index]['output']['consumer_api']['end']:
                unmatched.append({'address': address, 'present_qpc': row.qpc})
                continue
            entry = entries[index]
            if entry['csv'] is not None:
                raise ValueError('multiple CSV rows claim the same native Present call')
            entry['csv'] = row
            if entry['output']['outcome'] != 'Accepted':
                rejected.append({'token': entry['output']['token'], 'output': entry['output']['output'],
                                 'outcome': entry['output']['outcome'], 'dropped': row.dropped})
    uses = {(_key(r['token']), r['texture']): r for r in native['SourceUse']}
    frames = {_key(r['token']): r for r in native['FrameCpu']}
    published = {_key(r['identity']): r['observed_qpc'] for r in native['Plan'] if r['stage'] == 'Published'}
    producers = []
    for stream in sources:
        images = defaultdict(list)
        for row in stream:
            if row['kind'] == 'source_frame' and row['submission'].get('spout_image'):
                images[_key(row['submission']['spout_image'])].append(row)
        producers.append(images)
    try:
        query = report['live_query']
        snapshot = json.loads(query['stdout'])
        stats = snapshot['statistics']
        last_api = max((e['output']['consumer_api']['end'] for e in calls), default=0)
        live = query['exit_code'] == 0 and snapshot['status'] == 0 and snapshot['response_validation_error'] is None \
            and snapshot['session_name'] == report['session'] \
            and stats['returned_session_name'].casefold() == report['session'].casefold() \
            and last_api <= snapshot['before_qpc'] <= snapshot['after_qpc'] \
            and all(type(stats[k]) is int and stats[k] == 0
                    for k in ('events_lost', 'log_buffers_lost', 'realtime_buffers_lost'))
    except (KeyError, TypeError, ValueError):
        live = False
    loss = DATA['loss_observation'](report['stdout'], report.get('stderr', ''))
    healthy = live and loss['no_loss_reported'] and report.get('exit_code') == 0 \
        and not report.get('forced_cleanup') and not report.get('timed_out') and report.get('tools_unchanged') is True
    reports = []
    for output_id in (1, 3):
        accepted = [r for r in native['Output'] if r['output'] == output_id and r['outcome'] == 'Accepted'
                    and window['begin_qpc'] <= (r.get('delivery') or {}).get('end', -1) < window['end_qpc']]
        by_token = {_key(e['output']['token']): e for e in calls if e['output']['output'] == output_id}
        records, source_age, publication_age, plan_age, delays = [], [[], []], [[], []], [], []
        missing, dropped, undisplayed, missing_sources, missing_plans = 0, 0, 0, 0, 0
        for output in accepted:
            entry = by_token.get(_key(output['token']))
            row = entry['csv'] if entry else None
            record = {'token': output['token'], 'endpoint': output.get('window_present'),
                      'present_qpc': row.qpc if row else None, 'display_qpc': None,
                      'missing': None, 'source_age_ms': None}
            records.append(record)
            if row is None:
                missing += 1
                record['missing'] = 'no exact CSV row in native PID/address/API interval'
                continue
            if row.dropped or row.display_qpc is None:
                dropped += int(row.dropped)
                undisplayed += int(not row.dropped)
                record['missing'] = 'dropped' if row.dropped else 'display timing unavailable'
                continue
            displayed = row.display_qpc
            record['display_qpc'] = {'numerator': str(displayed.numerator), 'denominator': str(displayed.denominator)}
            delays.append(float((displayed-row.qpc)*1000/frequency))
            ages = []
            retained = []
            for index in range(2):
                use = uses.get((_key(output['token']), index+1))
                publication = use.get('publication') if use else None
                matches = producers[index].get(_key(publication.get('spout_image')), []) if publication else []
                if not use or not use['sequence'] or len(matches) != 1 or publication.get('publication_qpc') is None:
                    ages = []
                    break
                stamp = matches[0]['submission']['stamp']
                if stamp['source'] != index+1 or not 0 < stamp['qpc'] <= publication['publication_qpc'] <= displayed:
                    raise ValueError('source/publication/display clock or identity differs')
                ages.append(float((displayed-stamp['qpc'])*1000/frequency))
                retained.append(float((displayed-publication['publication_qpc'])*1000/frequency))
            if not ages:
                missing_sources += 1
                record['missing'] = 'no exact producer/source-use publication'
                continue
            record['source_age_ms'] = ages
            for index in range(2):
                source_age[index].append(ages[index])
                publication_age[index].append(retained[index])
            frame = frames.get(_key(output['token']))
            publication = published.get(_key(frame['generation']['identity'])) if frame else None
            if publication is not None and publication <= displayed:
                plan_age.append(float((displayed-publication)*1000/frequency))
            else:
                missing_plans += 1
                record['missing'] = 'no exact plan publication'
        # Every pre-window publication can still contribute to the observed backlog. A dropped
        # or unmatched frame has no measured consumption time and cannot certify the bound.
        events, pending, queue_complete = [], 0, True
        for output in native['Output']:
            if output['output'] != output_id or output['outcome'] != 'Accepted':
                continue
            api = output.get('consumer_api')
            if api is None:
                queue_complete = False
                continue
            if api['begin'] >= window['end_qpc']:
                continue
            entry = by_token.get(_key(output['token']))
            row = entry['csv'] if entry else None
            if row is None or row.display_qpc is None:
                queue_complete = False
                continue
            if row.display_qpc <= window['begin_qpc']:
                continue
            if row.qpc < window['begin_qpc']:
                pending += 1
            else:
                events.append((Fraction(row.qpc), 1))
            if row.display_qpc < window['end_qpc']:
                events.append((row.display_qpc, -1))
        peak = pending
        for _, change in sorted(events):
            pending += change
            peak = max(peak, pending)
        complete = bool(accepted) and missing+dropped+undisplayed+missing_sources == 0 \
            and len(plan_age) == len(accepted) and healthy and not unmatched
        reports.append({'output': output_id, 'expected_accepted_images': len(accepted),
                        'exact_present_rows': len(accepted)-missing, 'missing_rows': missing,
                        'dropped': dropped, 'display_time_unavailable': undisplayed, 'missing_sources': missing_sources,
                        'missing_plans': missing_plans,
                        'coverage_complete': complete, 'source_to_software_display_age': [distribution(v) for v in source_age],
                        'publication_to_software_display_age': [distribution(v) for v in publication_age],
                        'plan_publication_to_software_display_age': distribution(plan_age),
                        'present_to_software_display': distribution(delays),
                        'pending_software_display': {'coverage_complete': queue_complete and healthy and not unmatched,
                                            'observed_known_frame_peak': peak,
                                            'scope': 'known Present start to software display; not app/native buffer occupancy; unknown/drop retirement is not guessed'},
                        'records': records})
    return {'captured': True, 'coverage_complete': all(r['coverage_complete'] for r in reports),
            'csv_rows': total, 'unmatched_rows': unmatched, 'nonaccepted_native_rows': rejected,
            'collector_healthy': healthy, 'live_loss_query_covers_native_calls': live,
            'loss_observation': loss, 'outputs': reports,
            'scope': 'exact native PID/interface/API-interval to PresentMon v1 software display observations',
            'limitations': ['software display events do not prove physical scanout, pixel identity or optical latency',
                            'live loss counters do not include final collector shutdown; final warnings remain separate',
                            'GPU clocks/critical path and unknown dropped-frame consumption times remain unmeasured']}
