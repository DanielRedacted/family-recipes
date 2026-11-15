-- 009: canonical cuisines and categories tables
--
-- New cuisines/categories can only be added via: default values here,
-- imported structured JSON, or explicit user action.
-- AI parsing is constrained to only assign existing values.

CREATE TABLE IF NOT EXISTS cuisines (
  id   INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE IF NOT EXISTS categories (
  id   INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

INSERT OR IGNORE INTO cuisines (name) VALUES
  ('African'), ('American'), ('Asian'), ('Australian'), ('Brazilian'),
  ('British'), ('Cajun'), ('Caribbean'), ('Chinese'), ('Creole'),
  ('Cuban'), ('Ethiopian'), ('Filipino'), ('French'), ('German'),
  ('Greek'), ('Hawaiian'), ('Hungarian'), ('Indian'), ('Irish'),
  ('Israeli'), ('Italian'), ('Japanese'), ('Korean'), ('Lebanese'),
  ('Malaysian'), ('Mediterranean'), ('Mexican'), ('Middle Eastern'),
  ('Moroccan'), ('Pakistani'), ('Persian'), ('Polish'), ('Portuguese'),
  ('Russian'), ('Scandinavian'), ('South American'), ('Spanish'),
  ('Sri Lankan'), ('Thai'), ('Turkish'), ('Vietnamese');

INSERT OR IGNORE INTO categories (name) VALUES
  ('Appetizer'), ('Baked Good'), ('Beverage'), ('Bread'), ('Breakfast'),
  ('Brunch'), ('Cocktail'), ('Condiment'), ('Dessert'), ('Dip'),
  ('Drink'), ('Entrée'), ('Lunch'), ('Main Course'), ('Noodle'),
  ('Pasta'), ('Pastry'), ('Pickle'), ('Preserves'), ('Salad'),
  ('Sandwich'), ('Sauce'), ('Seafood'), ('Side Dish'), ('Snack'),
  ('Soup'), ('Starter'), ('Stew'), ('Stir-Fry'), ('Wrap');
