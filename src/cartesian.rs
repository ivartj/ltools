pub struct CartesianProduct<'a, E> {
    emptied: bool,
    vec: &'a Vec<Vec<E>>,
    counters: Vec<usize>,
}

pub fn cartesian_product<E>(vec: &Vec<Vec<E>>) -> CartesianProduct<E> {
    CartesianProduct{
        emptied: vec.is_empty() || vec.iter().any(Vec::is_empty),
        vec,
        counters: vec![0; vec.len()],
    }
}

impl<'a, E> Iterator for CartesianProduct<'a, E> {
    type Item = Vec<&'a E>;

    fn next(&mut self) -> Option<Vec<&'a E>> {
        if self.emptied {
            return None;
        }
        let v = self.counters.iter()
            .copied()
            .enumerate()
            .map(|(idx, counter)| &self.vec[idx][counter])
            .collect();

        // increment counters
        for (i, counter) in self.counters.iter_mut().enumerate().rev() {
            *counter += 1;
            if *counter != self.vec[i].len() {
                break;
            }
            *counter = 0;
            if i == 0 {
                self.emptied = true;
            }
        }

        Some(v)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_a() {
        let v0 = vec![1,2,3];
        let v1 = vec![4,5];
        let v = vec![v0, v1];
        assert_eq!(
            cartesian_product(&v).map(|v| v.into_iter().copied().collect()).collect::<Vec<Vec<i32>>>(),
            vec![
                vec![1,4], vec![1,5],
                vec![2,4], vec![2,5],
                vec![3,4], vec![3,5]]);
    }

    #[test]
    fn test_b() {
        let v0 = vec![1];
        let v1 = vec![];
        let v2 = vec![2];
        let v = vec![v0, v1, v2];
        assert_eq!(cartesian_product(&v).next(), None);
    }

    #[test]
    fn test_c() {
        let v0 = vec![1,2];
        let v1 = vec![3,4];
        let v2 = vec![5,6];
        let v = vec![v0, v1, v2];
        assert_eq!(
            cartesian_product(&v).map(|v| v.into_iter().copied().collect()).collect::<Vec<Vec<i32>>>(),
            vec![
                vec![1,3,5],
                vec![1,3,6],
                vec![1,4,5],
                vec![1,4,6],
                vec![2,3,5],
                vec![2,3,6],
                vec![2,4,5],
                vec![2,4,6]]);
    }

    #[test]
    fn test_d() {
        let v: Vec<Vec<i32>> = Vec::new();
        assert_eq!(cartesian_product(&v).next(), None);
    }
}

