use maplit::hashmap;
use rand::Rng;
use uuid::Uuid;

use crate::{common::config::Config, common::util::{percentage_based_output_int, float_range, generate_unemployed_salary}, game::{generation::{generate_education_level, get_expected_salary_range}}, as_decimal_percent, percentage_of};

use super::person::person::{EducationLevel::{*, self}, Person, Job};

#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub enum ProductType {
    #[default]
    Leisure,

    // These will be implemented later:
    // FURNITURE
    // HOUSES
}

#[derive(Default, Clone)]
pub struct Business {
    pub id: Uuid,

    pub balance: f64,

    pub minimum_education_level: EducationLevel,
    pub expected_marketing_reach: i32, // Amount of population that the marketing will reach (roughly)
    pub product_price: i32,
    pub production_cost_per_product: f32,
    pub marketing_cost_percentage: i32,
    pub product_type: ProductType,
    
    pub employee_salary: i32,
    pub employees: Vec<Uuid>,
    pub employee_budget_allocation: f32,

    pub expected_income: i64, // Expected income for the current month
    pub last_month_balance: f64, // Used to calculate the income for this month

    pub removal_queue: Vec<usize>,
}

impl Business {
    /// Generates a business based on demand
    pub fn generate(&mut self, config: &Config, product_type: ProductType, product_demand: f32, remaining_market_percentage: &mut f32, people: &mut Vec<Person>, idx: usize, tax_rate: f32) -> bool {
        self.id = Uuid::new_v4();
        let mut rng = rand::thread_rng();

        self.product_type = product_type;
        self.minimum_education_level = generate_education_level(config);
        self.marketing_cost_percentage = rng.gen_range(5..12);
        self.product_price = rng.gen_range(2..100); // TODO: determine this price more accurately?
        self.production_cost_per_product = self.product_price as f32 * float_range(0.15, 0.25, 3);

        let (sufficient_businesses, marketing_reach_percentage) = self.generate_marketing_reach(remaining_market_percentage);
        if sufficient_businesses { return sufficient_businesses }

        let exp_purchases = self.assign_to_people(marketing_reach_percentage, people) as i64;
        self.expected_income = exp_purchases * self.product_price as i64;

        // TODO: make this more varied & accurate, influence it by external factors
        let production_cost = self.get_production_cost();
        let marketing_cost = as_decimal_percent!(self.marketing_cost_percentage) * self.expected_income as f32;
        // This can only be a maximum of 67%, leaving roughly 30% capacity for employees, the minimum (with tax no lower than 20%) is 40%
        let mut loss_percentage = percentage_of!(marketing_cost + production_cost as f32; / self.expected_income) + (tax_rate * 100.) as i32;

        self.employee_salary = self.generate_employee_salary(config, loss_percentage);
        self.employee_budget_allocation = float_range(0.15, 0.3, 3);

        let expected_employee_count = self.calculate_expected_employee_count();
        self.assign_employees(people, expected_employee_count);
        
        loss_percentage += percentage_of!(self.employees.len() * (self.employee_salary as usize / 12); / self.expected_income);
        self.set_starting_balance(loss_percentage);

        false
    }

    fn set_starting_balance(&mut self, loss_percentage: i32) {
        let expected_income = self.expected_income as f64;

        self.balance = expected_income * float_range(0.15, 3., 3) as f64; // A range of 0% - 300% of the expected profit is the business balance
        self.balance -= expected_income * as_decimal_percent!(loss_percentage) as f64;
        self.last_month_balance = self.balance;
    }

    fn calculate_expected_employee_count(&self) -> i32 {
        let budget_allocation = (self.expected_income as f32 * self.employee_budget_allocation) as i32;
        (budget_allocation as f32 / (self.employee_salary as f32 / 12.)) as i32
    }

    fn generate_employee_salary(&self, config: &Config, loss_percentage: i32) -> i32 {
        let expected_salary_range = get_expected_salary_range(config, &self.minimum_education_level);

        let mid_of_range = (expected_salary_range.start + expected_salary_range.end) / 2; // middle of expected salary range
        let lower_mid_of_range = expected_salary_range.start + ((expected_salary_range.end - mid_of_range) / 2); // lower middle of expected salary range

        // Generate a more narrowed down range based on the randomly generated marketing cost and product cost
        // The employee salary will be lower if the marketing and product cost is cheap, this is to compensate and keep the economy balanced
        // The purpose of lowering the employee salary is that there is less work output from employees, so there is a higher staff turnover
        let employee_salary_range = match loss_percentage {
            loss if loss >= 60 => mid_of_range..expected_salary_range.end,
            loss if loss >= 45 => lower_mid_of_range..mid_of_range,
            _ => expected_salary_range.start..lower_mid_of_range,
        };

        rand::thread_rng().gen_range(employee_salary_range)
    }

    pub fn get_production_cost(&self) -> f32 {
        (self.expected_income / self.product_price as i64) as f32 * self.production_cost_per_product as f32
    }

    fn generate_marketing_reach(&self, remaining_market_percentage: &mut f32) -> (bool, f32) {
        let marketing_reach_percentage = match self.minimum_education_level {
            NoFormalEducation => self.random_marketing_percentage_multiplyer(0.3, 0.5),
            HighSchoolDiploma => self.random_marketing_percentage_multiplyer(0.5, 0.9),
            College => self.random_marketing_percentage_multiplyer(0.6, 1.1),
            AssociateDegree => self.random_marketing_percentage_multiplyer(0.8, 1.4),
            Bachelors => self.random_marketing_percentage_multiplyer(1., 2.1),
            AdvancedDegree => self.random_marketing_percentage_multiplyer(0.5, 4.),
        };

        if (*remaining_market_percentage - marketing_reach_percentage) < 0. {
            return (true, marketing_reach_percentage);
        }

        *remaining_market_percentage -= marketing_reach_percentage;
        (false, marketing_reach_percentage)
    }

    pub fn assign_to_people(&self, market_percentage: f32, people: &mut Vec<Person>) -> i32 {
        let mut rng = rand::thread_rng();
        let reach = ((market_percentage / 100.) * people.len() as f32) as i32;

        // People who have not yet picked a business to buy from
        let unassigned_people: Vec<&mut Person> = people.iter_mut().filter(|p| p.business_this_month.is_none()).collect(); // TODO: optimise this
        let mut expected_purchases = 0; 

        for (count, person) in unassigned_people.into_iter().enumerate() {
            if count == reach as usize { break }

            person.business_this_month = Some(self.id);
            let person_demand = person.demand[&self.product_type];
            let purchase_capacity = person_demand as i32 / self.product_price;
            expected_purchases += purchase_capacity;

            for _ in 0..purchase_capacity {
                let day = rng.gen_range(1..=30);
                *person.purchase_days.entry(day).or_insert(1) += 1;
            }
        }

        (expected_purchases as f32 * 0.95) as i32 // Expect roughly 5% of people not afford items
    }

    fn assign_employees(&mut self, people: &mut [Person], new_employee_count: i32) {
        let minimum_education_level = self.minimum_education_level.clone();
        let unemployed_people: Vec<&mut Person> = people.iter_mut().filter(|p| {
            p.job == Job::Unemployed && p.education_level == minimum_education_level && p.age >= 18
        }).collect(); // TODO: optimise this

        for (count, person) in unemployed_people.into_iter().enumerate() {
            if count == new_employee_count as usize { break }
            self.employees.push(person.id);

            person.job = Job::Employee(self.id);
            person.salary = self.employee_salary;
        }
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

    /// This function assigns the business to a new market with a new market percentage. This runs monthly.
    pub fn get_new_market(&mut self, market_percentage: f32, cost_per_percent: f32, people: &mut Vec<Person>, demand: f32) {
        self.expected_income = self.assign_to_people(market_percentage, people) as i64 * self.product_price as i64;
        let employee_diff = self.calculate_expected_employee_count() - self.employees.len() as i32;

        if employee_diff > 0 {
            self.assign_employees(people, employee_diff);
        } else if employee_diff < 0 {
            self.remove_employees(employee_diff, people);
        }

        self.balance -= (self.get_production_cost() + (market_percentage * cost_per_percent)) as f64;
    }

    pub fn remove_employees(&mut self, amount: i32, people: &mut Vec<Person>) {
        // Sort employees by lowest welfare to highest

        let mut sorted_employees: Vec<_> = self.employees.clone().into_iter().collect();
        sorted_employees.sort_by(|a, b| {
            let per_a = people.iter().find(|p| p.id == *a).unwrap();
            let per_b = people.iter().find(|p| p.id == *b).unwrap();

            per_a.welfare.cmp(&per_b.welfare)
        });

        for _ in 0..amount {
            let per_id = sorted_employees.remove(0);

            let emp_idx = self.employees.iter().position(|emp_id| *emp_id == per_id).unwrap();
            self.employees.remove(emp_idx);

            let per = people.iter_mut().find(|p| p.id == per_id).unwrap();
            per.job = Job::Unemployed;
            per.salary = generate_unemployed_salary();
        }
    }

    /// This is run on a monthly basis
    pub fn pay_owner(&mut self, owner: &mut Person) {
        let month_profits = self.balance - self.last_month_balance; // The profit percentage earned in the current month

        let owner_expected_income = month_profits / 2.;
        if owner_expected_income < (self.employee_salary as f64 / 12.) {
            owner.business_pay(self, self.employee_salary as f64);
            return;
        }

        owner.business_pay(self, owner_expected_income);
    }

    pub fn pay_tax(&mut self, government_balance: &mut i64, amount: f64) {
        if amount < 0. { return }
        self.balance -= amount as f64;
        *government_balance += amount as i64;
    }
}