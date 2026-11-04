"""Bounded event burst from an owned fake provider; no production sessions."""

SOURCE = r'''
def watch_gap():
    while True:
        if (root/'emit-event-gap').exists():
            (root/'emit-event-gap').unlink()
            with lock:
                history=turns.setdefault('target',[])
                sequence=1+sum(t['id'].startswith('gap-fixture-turn-') for t in history)
                turn_id='gap-fixture-turn-'+str(sequence)
                item=dict(id=turn_id+'-answer',type='agentMessage',text='Recovered missed events '+str(sequence)+' 中文😀')
                history.append(dict(id=turn_id,status='completed',items=[item]));save()
                if 'SOURCES' in globals():SOURCES.append(item)
                noise=json.dumps(dict(method='studio/appServer/log',params=dict(line='private-gap-noise:'+'x'*4096)))
                for index in range(8192):
                    print(noise,flush=True)
                    if index==4096:
                        print(json.dumps(dict(method='item/completed',params=dict(threadId='target',turnId=turn_id,item=item))),flush=True)
            (root/'gap-published.json').write_text(json.dumps(dict(sequence=sequence,events=8192,lineBytes=4096)))
        time.sleep(.02)
threading.Thread(target=watch_gap,daemon=True).start()
'''

def instrument(source, events=8192):
    assert events in (8192, 16384), 'Only bounded private fixture sizes are allowed'
    assert source.count('for raw in sys.stdin:') == 1
    burst=SOURCE.replace('range(8192)',f'range({events})').replace('events=8192',f'events={events}')
    return source.replace('for raw in sys.stdin:', burst + '\nfor raw in sys.stdin:', 1)
