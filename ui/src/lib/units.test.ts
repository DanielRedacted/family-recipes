import { describe, expect, it } from 'vitest';
import { normalizeUnits as n } from './units';

describe('normalizeUnits', () => {
  it('writes each unit one way', () => {
    expect(n('1 Tablespoon honey, 1 TB oil, 1 T butter, 1 Tbsp. sugar, 3 tbs milk, 3 to 4 Tbps butter')).toBe('1 tbsp honey, 1 tbsp oil, 1 tbsp butter, 1 tbsp sugar, 3 tbsp milk, 3 to 4 tbsp butter');
    expect(n('½ teaspoon salt, 1 t vanilla, 2 ts pepper, 1 tsp. salt, 1 TSP mustard')).toBe('½ tsp salt, 1 tsp vanilla, 2 tsp pepper, 1 tsp salt, 1 tsp mustard');
    expect(n('2 cup flour, 1 ½ cup sugar, ½ cups milk, 6 c oil, 2 CUPS FLOUR, 1-2 cup water, ½ to 1 cups cream')).toBe('2 cups flour, 1 ½ cups sugar, ½ cup milk, 6 cups oil, 2 cups FLOUR, 1-2 cups water, ½ to 1 cup cream');
    expect(n('8 ounces cheese, 10 oz. milk, 3 lbs. beef, 4 LB roast, 2 pounds pork, 1 pound sugar')).toBe('8 oz cheese, 10 oz milk, 3 lb beef, 4 lb roast, 2 lb pork, 1 lb sugar');
    expect(n('3 quarts broth, 2 qt. dish, 1 pint cream, 1 gallon water, 454 grams flour, 180 mL water, 2 liters, 8 fl. oz juice')).toBe('3 qt broth, 2 qt dish, 1 pt cream, 1 gal water, 454 g flour, 180 ml water, 2 l, 8 fl oz juice');
  });
  it('keeps everything else as written', () => {
    expect(n('2 cloves garlic, 1 can soup, pinch salt, 1 stick butter, 2 dozen eggs')).toBe('2 cloves garlic, 1 can soup, pinch salt, 1 stick butter, 2 dozen eggs');
    expect(n('add a tablespoon of oil, then the cups')).toBe('add a tablespoon of oil, then the cups');
    expect(n('1 (10.75 ounce) can soup')).toBe('1 (10.75 oz) can soup');
    expect(n('2 Tbsp cup lemon juice')).toBe('2 tbsp cup lemon juice');
    expect(n('a 9x13 pan, 9” pie, Step 2 then')).toBe('a 9x13 pan, 9” pie, Step 2 then');
  });
  it('spaces, adjectives, bold and doubled units', () => {
    expect(n('11oz bottles, 2lbs. beans, 1TB peel, 180ml water')).toBe('11 oz bottles, 2 lb beans, 1 tbsp peel, 180 ml water');
    expect(n('1/8 heaping teaspoon salt, 1 scant tsp. soda')).toBe('1/8 heaping tsp salt, 1 scant tsp soda');
    expect(n('**1 ½** tablespoon sugar')).toBe('**1 ½** tbsp sugar');
    expect(n('½ c cup dried cherries')).toBe('½ cup dried cherries');
  });
  it('keeps a period that ends a sentence', () => {
    expect(n('Add 1 tsp.')).toBe('Add 1 tsp.');
    expect(n('Add 1 tsp. Mix well.')).toBe('Add 1 tsp. Mix well.');
    expect(n('Add 1 tsp. salt per cup')).toBe('Add 1 tsp salt per cup');
    expect(n('- 1 tsp. Salt\n1 – 2 lb. Chicken & 64 oz. Chicken broth\n| 2 Tbsp. | Butter |')).toBe('- 1 tsp Salt\n1 – 2 lb Chicken & 64 oz Chicken broth\n| 2 tbsp | Butter |');
  });
  it('temperatures with a scale, fraction suffixes', () => {
    expect(n('Preheat to 425 degrees F (190 degrees C). Fry at 350F (175C), 350 deg F, 350° F, 325 °F, 105-115ºF, 350 degrees Fahrenheit')).toBe(
      'Preheat to 425°F (190°C). Fry at 350°F (175°C), 350°F, 350°F, 325°F, 105-115°F, 350°F');
    expect(n('Bake at 350°. Fry at 300 degrees. Heat to 350º.')).toBe('Bake at 350°. Fry at 300 degrees. Heat to 350°.');
    expect(n('1/8th tsp salt; bring 2/3rds of it; the 3rd day')).toBe('1/8 tsp salt; bring 2/3 of it; the 3rd day');
  });
  it('leaves link addresses and names alone', () => {
    expect(n('[1 cup sugar](http://x.com/1-cup-sugar) and [[2 Tbsp Sauce]] http://y.com/3c')).toBe('[1 cup sugar](http://x.com/1-cup-sugar) and [[2 Tbsp Sauce]] http://y.com/3c');
  });
});
