//! A friendly name for a device, as LocalSend makes them: an adjective and
//! a fruit ("Juicy Orange", «Swift Mango»). Other devices see it in the
//! list of devices nearby and while pairing; the user can change it.

use rand::Rng;

#[derive(Debug, Clone, Copy)]
enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

/// Russian adjectives in the masculine, feminine and neuter forms.
const RU_ADJECTIVES: &[(&str, &str, &str)] = &[
    ("Сочный", "Сочная", "Сочное"),
    ("Спелый", "Спелая", "Спелое"),
    ("Сладкий", "Сладкая", "Сладкое"),
    ("Свежий", "Свежая", "Свежее"),
    ("Яркий", "Яркая", "Яркое"),
    ("Тихий", "Тихая", "Тихое"),
    ("Быстрый", "Быстрая", "Быстрое"),
    ("Смелый", "Смелая", "Смелое"),
    ("Хитрый", "Хитрая", "Хитрое"),
    ("Весёлый", "Весёлая", "Весёлое"),
    ("Мудрый", "Мудрая", "Мудрое"),
    ("Добрый", "Добрая", "Доброе"),
    ("Тайный", "Тайная", "Тайное"),
    ("Золотой", "Золотая", "Золотое"),
    ("Лесной", "Лесная", "Лесное"),
    ("Летний", "Летняя", "Летнее"),
    ("Солнечный", "Солнечная", "Солнечное"),
    ("Бодрый", "Бодрая", "Бодрое"),
    ("Гордый", "Гордая", "Гордое"),
    ("Нежный", "Нежная", "Нежное"),
];

const RU_FRUITS: &[(&str, Gender)] = &[
    ("Апельсин", Gender::Masculine),
    ("Банан", Gender::Masculine),
    ("Лимон", Gender::Masculine),
    ("Ананас", Gender::Masculine),
    ("Персик", Gender::Masculine),
    ("Абрикос", Gender::Masculine),
    ("Мандарин", Gender::Masculine),
    ("Арбуз", Gender::Masculine),
    ("Виноград", Gender::Masculine),
    ("Гранат", Gender::Masculine),
    ("Груша", Gender::Feminine),
    ("Слива", Gender::Feminine),
    ("Вишня", Gender::Feminine),
    ("Дыня", Gender::Feminine),
    ("Малина", Gender::Feminine),
    ("Клубника", Gender::Feminine),
    ("Черешня", Gender::Feminine),
    ("Хурма", Gender::Feminine),
    ("Яблоко", Gender::Neuter),
    ("Манго", Gender::Neuter),
    ("Киви", Gender::Neuter),
];

const EN_ADJECTIVES: &[&str] = &[
    "Juicy", "Ripe", "Sweet", "Fresh", "Bright", "Quiet", "Swift", "Brave", "Clever", "Happy",
    "Wise", "Kind", "Secret", "Golden", "Wild", "Sunny", "Lucky", "Proud", "Gentle", "Calm",
];

const EN_FRUITS: &[&str] = &[
    "Orange",
    "Banana",
    "Lemon",
    "Pineapple",
    "Peach",
    "Apricot",
    "Mango",
    "Melon",
    "Grape",
    "Pear",
    "Plum",
    "Cherry",
    "Raspberry",
    "Strawberry",
    "Apple",
    "Kiwi",
    "Papaya",
    "Fig",
    "Lime",
    "Coconut",
];

/// A new name in Russian (`ru`) or English.
pub fn generate(ru: bool, rng: &mut impl Rng) -> String {
    if ru {
        let (masculine, feminine, neuter) = RU_ADJECTIVES[rng.gen_range(0..RU_ADJECTIVES.len())];
        let (fruit, gender) = RU_FRUITS[rng.gen_range(0..RU_FRUITS.len())];
        let adjective = match gender {
            Gender::Masculine => masculine,
            Gender::Feminine => feminine,
            Gender::Neuter => neuter,
        };
        format!("{adjective} {fruit}")
    } else {
        let adjective = EN_ADJECTIVES[rng.gen_range(0..EN_ADJECTIVES.len())];
        let fruit = EN_FRUITS[rng.gen_range(0..EN_FRUITS.len())];
        format!("{adjective} {fruit}")
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    #[test]
    fn russian_adjectives_agree_with_the_fruit() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..500 {
            let name = generate(true, &mut rng);
            let (adjective, fruit) = name.split_once(' ').expect("two words");
            let (_, gender) = RU_FRUITS
                .iter()
                .find(|(known, _)| *known == fruit)
                .expect("known fruit");
            let ending_ok = match gender {
                Gender::Masculine => {
                    adjective.ends_with("ый")
                        || adjective.ends_with("ий")
                        || adjective.ends_with("ой")
                }
                Gender::Feminine => adjective.ends_with("ая") || adjective.ends_with("яя"),
                Gender::Neuter => adjective.ends_with("ое") || adjective.ends_with("ее"),
            };
            assert!(ending_ok, "{name}");
        }
    }

    #[test]
    fn names_are_two_words_and_vary() {
        let mut rng = StdRng::seed_from_u64(1);
        let names: std::collections::BTreeSet<String> =
            (0..50).map(|_| generate(false, &mut rng)).collect();
        assert!(names.len() > 20);
        assert!(names.iter().all(|name| name.split(' ').count() == 2));
    }
}
