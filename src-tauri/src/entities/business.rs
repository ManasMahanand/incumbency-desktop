use maplit::hashmap;
use rand::Rng;

use crate::{generation::{generate_education_level, get_expected_salary_range}, config::Config, util::{percentage_based_output_int, float_range}};
use super::person::{EducationLevel::{*, self}, Person, Job};

#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub enum ProductType {
    #[default]
    LEISURE,

    // These will be implemented later:
    // FURNITURE
    // HOUSES
}

#[derive(Default)]
pub struct Business {
    pub minimum_education_level: EducationLevel,
    pub expected_marketing_reach: i32, // Amount of population that the marketing will reach (roughly)
    pub product_price: f32,
    pub production_cost: f32,
    
    pub employee_salary: i32,
    pub default_employee_profit_percentage: i32, // Default percentage of profit that is made from an employee salary, not taking into account the employee's welfare
}

impl Business {
    /// Generates a business based on demand
    pub fn generate(&mut self, config: &Config, product_type: ProductType, product_demand: f32, remaining_market_percentage: &mut f32, people: &mut Vec<Person>, idx: usize) -> bool {
        self.minimum_education_level = generate_education_level(&config);

        let marketing_reach_percentage = match self.minimum_education_level {
            NoFormalEducation => self.random_marketing_percentage_multiplyer(0.3, 0.5),
            HighSchoolDiploma => self.random_marketing_percentage_multiplyer(0.5, 0.9),
            College => self.random_marketing_percentage_multiplyer(0.6, 1.1),
            AssociateDegree => self.random_marketing_percentage_multiplyer(0.8, 1.4),
            Bachelors => self.random_marketing_percentage_multiplyer(1., 2.1),
            AdvancedDegree => self.random_marketing_percentage_multiplyer(0.5, 4.),
        } as f32;

        if (*remaining_market_percentage - marketing_reach_percentage) < 0. {
            return true;
        }

        *remaining_market_percentage -= marketing_reach_percentage;

        let mut rng = rand::thread_rng();

        // TODO: determine this price more accurately
        self.product_price = rng.gen_range(2..100) as f32;
        
        let expected_income = product_demand * marketing_reach_percentage;

        // TODO: make this more varied & accurate
        self.production_cost = self.product_price * float_range(0.4, 0.5, 3);

        let marketing_cost = product_demand * float_range(0.1, 0.3, 3);
        let expected_salary_range = get_expected_salary_range(&config, &self.minimum_education_level);

        // This can only be a maximum of 80%, leaving roughly 10% capacity for employees, the minimum is 50%
        let loss_percentage_before_employees = ((marketing_cost + self.production_cost) / expected_income) * 100.;
        let mid_of_range = (expected_salary_range.start + expected_salary_range.end) / 2;
        let lower_mid_of_range = expected_salary_range.start + ((expected_salary_range.end - mid_of_range) / 2);

        let employee_salary_range = match loss_percentage_before_employees {
            loss if loss >= 70. => mid_of_range..expected_salary_range.end,
            loss if loss >= 60. => lower_mid_of_range..mid_of_range,
            _ => expected_salary_range.start..lower_mid_of_range,
        };

        self.employee_salary = rng.gen_range(employee_salary_range);
        self.default_employee_profit_percentage = rng.gen_range(8..11);

        let employee_monthly_salary = self.employee_salary / 12;
        let deducted_income = expected_income - (expected_income * (loss_percentage_before_employees / 100.));
        let employee_count = deducted_income as i32 / (employee_monthly_salary + (employee_monthly_salary * (self.default_employee_profit_percentage / 100)));

        let minimum_education_level = self.minimum_education_level.clone();
        let unemployed_people: Vec<&mut Person> = people.iter_mut().filter(|p| {
            p.job == Job::Unemployed && p.education_level == minimum_education_level
        }).collect(); // TODO: optimise this

        let mut count = 0;
        for person in unemployed_people {
            if count == employee_count { continue }

            person.job = Job::Employee(idx);
            count += 1;
        }

        false
    }


    /// Multiplies the percentage target audience for the market based on educated odds 
    pub fn random_marketing_percentage_multiplyer(&self, min: f32, max: f32) -> f32 {
        // 1 - smallest, 3 - largest
        let tier = percentage_based_output_int(hashmap! {
            1 => 75,
            2 => 20,
            3 => 5,
        });

        let mut rng = rand::thread_rng();
        let increase_multiplyer = match tier {
            8 => rng.gen_range(2..5) as f32, // Increase start and end by a random range of 150%-320%
            3 => rng.gen_range(5..10) as f32,
            _ => 1.,
        };

        float_range(min * increase_multiplyer, max * increase_multiplyer, 2)
    }
}