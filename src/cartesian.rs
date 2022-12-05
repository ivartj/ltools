pub struct CartesianProduct<'a, E> {
    empty: bool,
    vec: &'a Vec<Vec<E>>,
    counters: Vec<usize>,
}

pub fn cartesian_product<'a, E>(vec: &'a Vec<Vec<E>>) -> CartesianProduct<'a, E> {
    CartesianProduct{
        empty: vec.iter().any(|v| v.len() == 0),
        vec,
        counters: vec![0; vec.len()],
    }
}

impl<'a, E> Iterator for CartesianProduct<'a, E> {
    type Item = Vec<&'a E>;

    fn next(&mut self) -> Option<Vec<&'a E>> {
        if self.empty {
            return None;
        }

        let retval = self.counters.iter()
            .copied()
            .enumerate()
            .map(|(i, counter)| &self.vec[i][counter])
            .collect();

        // increment counters
        for (i, counter) in self.counters.iter_mut().enumerate().rev() {
            *counter += 1;
            if *counter == self.vec[i].len() {
                if i == 0 {
                    self.empty = true;
                } else {
                    *counter = 0;
                }
                continue;
            } else {
                break;
            }
        }

        Some(retval)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_a() {
        let v = vec![vec![1,2,3], vec![4,5]];
        assert_eq!(
            cartesian_product(&v).map(|v| v.into_iter().copied().collect()).collect::<Vec<Vec<i32>>>(),
            vec![
                vec![1,4], vec![1,5],
                vec![2,4], vec![2,5],
                vec![3,4], vec![3,5]]);
    }

    #[test]
    fn test_b() {
        let v = vec![vec![1], vec![], vec![2]];
        assert_eq!(cartesian_product(&v).next(), None);
    }

    #[test]
    fn test_c() {
        let v = vec![vec![1,2], vec![3,4], vec![5,6]];
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
}

