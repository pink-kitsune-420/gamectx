"""Local dialogue trade prototype: host prices, explicit one-use confirmation."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_reference import need,pair
from dungeon_slots_reference import tokens,resolve
from dungeon_knowledge_reference import stock_question
class Trade:
    def __init__(self):self.pending=None;self.sequence=0
    def cancel(self):self.pending=None;return {'cancelled':True}
    def exchange(self,g,speaker,text):
        need(isinstance(text,str) and 0<len(text.strip())<=240 and not any(ord(c)<32 for c in text))
        need(speaker in ['merchant','keeper','narrator'] and bool(g.choices()))
        need(speaker=='narrator' or g.room==('workshop' if speaker=='merchant' else 'cistern'))
        if speaker!='merchant':self.pending=None;return {'handled':False}
        words=tokens(text);yes=words in [['yes'],['yes','please'],['y'],['confirm']];no=words in [['no'],['no','thanks'],['n'],['cancel']]
        if yes or no:
            offer=self.pending;self.pending=None
            if offer is None:return {'handled':True,'reply':'No purchase is waiting. Tell me which item you would like to buy.'}
            if no:return {'handled':True,'reply':'Purchase cancelled. Your coins and inventory are unchanged.'}
            if offer['revision']!=g.revision or offer['decision'] not in g.choices():return {'handled':True,'reply':'That offer is no longer current. Ask to buy the item again.'}
            item=next(i for i in g.public()['items'] if i['id']==offer['decision']['target'])
            if item['price']!=offer['terms']['price']:return {'handled':True,'reply':'That offer is no longer current. Ask to buy the item again.'}
            result=g.act(g.revision,offer['decision']);need(result['status']=='accepted')
            return {'handled':True,'reply':'You bought '+item['label']+' for '+str(item['price'])+' coins. You have '+str(g.coins)+' coins left.','result':result}
        self.pending=None
        negative=any(w in words for w in ['not','don','never','without','cannot']) or "can't" in text.lower()
        request=any(w in words for w in ['buy','purchase']) or text.lower().startswith(("i'd like ",'i would like '))
        if negative or not request:return {'handled':False}
        items=g.public()['items'];pairs=[pair('buy',i['id']) for i in items]
        r=resolve(pairs,items,text)
        if stock_question(text) and not r['candidates']:return {'handled':False}
        if len(r['candidates'])!=1:return {'handled':True,'reply':'Which item would you like to buy? Name one visible item.'}
        id=r['candidates'][0]['id'];item=next(i for i in items if i['id']==id)
        if item['owner']!='merchant':return {'handled':True,'reply':'That item is not for sale by this merchant.'}
        price=item['price']
        if price>g.coins:return {'handled':True,'reply':'The '+item['label']+' costs '+str(price)+' coins. You have '+str(g.coins)+', so you need '+str(price-g.coins)+' more.'}
        if pair('buy',id) not in g.choices():return {'handled':True,'reply':'That item cannot be bought right now.'}
        self.sequence+=1;self.pending={'id':'trade_'+str(self.sequence),'scope':'merchant','revision':g.revision,'decision':pair('buy',id),'terms':{'item':id,'price':price,'quantity':1}}
        return {'handled':True,'reply':'The '+item['label']+' costs '+str(price)+' coins. You have '+str(g.coins)+'. Buy one? Say yes or no.','offer':copy.deepcopy(self.pending)}
