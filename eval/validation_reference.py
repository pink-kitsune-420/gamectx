"""Independent structural/number expectations, defined before Rust optimization."""
import json,re
NUMBER=re.compile(r'-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?\Z')
def numbers():
 valid=['0','-0','1','-12','1.50','1e999999','1E+12','-2.3e-10','123456789012345678901234567890']
 invalid=['','+1','00','01','-','-.1','1.','1e','NaN','inf',' 1','1 ','1x','1e+','--1','١']
 return [dict(text=x,valid=bool(NUMBER.fullmatch(x))) for x in valid+invalid]
if __name__=='__main__':print(json.dumps(dict(number_cases=numbers(),api_spend_usd=0,model_calls=0)))
