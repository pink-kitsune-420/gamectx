"""Predeclared focused visible-state policy, independent of example host code."""
def visible(input,room,items,coins,expanded=False,unknown=False):
 if unknown:return None
 if expanded:return 'host_public_state'
 text=input.lower();stock=any(w in text for w in ['stock','sell','offer'])
 return dict(room=room,items=[i for i in items if stock and i['owner']=='merchant' or i['label'] in text or i['id'].split(':')[-1].replace('_',' ') in text],coins=coins if stock else None)
def route_size(size):return size.rstrip('bB')+'b'
if __name__=='__main__':
 assert all(route_size(v)=='3b' for v in ['3','3b','3B'])
 items=[dict(id='item:oil',label='hinge oil',owner='merchant'),dict(id='item:salve',label='salve',owner='player')]
 assert visible('Do you trust me?','workshop',items,3)['items']==[]
 assert visible('What stock is listed?','workshop',items,3)['items']==[items[0]]
 assert visible('Tell me about oil','workshop',items,3)['items']==[items[0]]
 assert visible('Where was I born?','workshop',items,3,unknown=True) is None
 print('Focused context prototype: four checks, zero provider calls')
